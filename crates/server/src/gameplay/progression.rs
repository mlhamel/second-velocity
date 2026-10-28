use super::*;
use sv_protocol::{BuyOutfit, BuyShip, SellOutfit};

pub(super) fn weapon_banks(pack: &ContentPack, ids: &[String]) -> WeaponBanks {
    WeaponBanks(
        ids.iter()
            .filter_map(|id| match &pack.outfit(id)?.kind {
                OutfitKind::Weapon(def) => Some(EquippedWeapon {
                    def: def.clone(),
                    state: WeaponState::new(),
                }),
                _ => None,
            })
            .collect(),
    )
}

pub(super) fn restore_ship(
    pack: &ContentPack,
    saved: &crate::PlayerState,
) -> Result<(ShipState, Vec<String>), String> {
    let def = pack
        .ship(&saved.ship_id)
        .ok_or_else(|| format!("Saved hull '{}' is unavailable.", saved.ship_id))?;
    let ids = saved
        .equipped_outfits
        .clone()
        .unwrap_or_else(|| def.default_outfits.clone());
    let stats = pack.loadout_stats(&saved.ship_id, &ids)?;
    if ![
        saved.position.0,
        saved.position.1,
        saved.fuel,
        saved.shields,
        saved.armor,
    ]
    .into_iter()
    .all(f32::is_finite)
        || pack.system(&saved.system).is_none()
    {
        return Err("Saved ship state is invalid.".into());
    }
    let mut state = ShipState::new(stats.max_shields, stats.max_armor, stats.max_fuel);
    state.position = Vec2::from(saved.position);
    // Legacy death snapshots stored zero armor; commission their recovered hull.
    if saved.armor > 0.0 {
        state.shields = saved.shields.clamp(0.0, state.max_shields);
        state.armor = saved.armor.clamp(0.0, state.max_armor);
        state.fuel = saved.fuel.clamp(0.0, state.max_fuel);
    }
    Ok((state, ids))
}

pub(super) fn spawn_saved_player(
    commands: &mut Commands,
    pack: &ContentPack,
    owner: u64,
    client: Entity,
    name: String,
    saved: &crate::PlayerState,
) -> Result<Entity, String> {
    let (state, ids) = restore_ship(pack, saved)?;
    let def = pack
        .ship(&saved.ship_id)
        .ok_or("Owned hull is unavailable.")?;
    let entity = spawn_ship(
        commands,
        pack,
        &saved.ship_id,
        ShipKind::Player,
        Some(owner),
        &saved.system,
        Vec2::ZERO,
    );
    commands.entity(entity).insert((
        Position(state.position),
        Hull {
            shields: state.shields,
            max_shields: state.max_shields,
            armor: state.armor,
            max_armor: state.max_armor,
        },
        Fuel {
            current: state.fuel,
            max: state.max_fuel,
        },
        Ship(state),
        weapon_banks(pack, &ids),
        EquippedOutfits { ids },
        Wallet(Account {
            credits: saved.credits,
            cargo: saved.cargo.clone(),
        }),
        AccountState {
            credits: saved.credits,
            cargo: saved.cargo.clone(),
            cargo_capacity: def.cargo,
        },
        parse_active_quests(&saved.active_quests_json),
        super::factions::parse_reputation(&saved.reputation_json),
        ClientRef(client),
        PlayerName(name),
    ));
    Ok(entity)
}

type ShopShips<'w, 's> = Query<
    'w,
    's,
    (
        &'static Owner,
        &'static DockedAt,
        &'static mut Ship,
        &'static mut ShipSpec,
        &'static mut ShipInfo,
        &'static mut EquippedOutfits,
        &'static mut WeaponBanks,
        &'static mut Hull,
        &'static mut Fuel,
        &'static mut Wallet,
        &'static mut AccountState,
        Option<&'static PlayerName>,
        &'static ActiveQuests,
        Option<&'static sv_protocol::PlayerReputation>,
    ),
    With<PlayerShip>,
>;

enum Purchase<'a> {
    Ship(&'a str),
    BuyOutfit(&'a str),
    SellOutfit(&'a str),
}

pub(super) fn on_buy_ship(
    event: On<FromClient<BuyShip>>,
    mut commands: Commands,
    clients: Query<&NetworkId>,
    content: Res<Content>,
    storage: Res<StorageResource>,
    mut ships: ShopShips,
) {
    let owner = raw_id(event.client_id, &clients);
    purchase(
        Purchase::Ship(&event.event().ship),
        owner,
        event.client_id,
        &mut commands,
        &content.0,
        &storage,
        &mut ships,
    );
}

pub(super) fn on_buy_outfit(
    event: On<FromClient<BuyOutfit>>,
    mut commands: Commands,
    clients: Query<&NetworkId>,
    content: Res<Content>,
    storage: Res<StorageResource>,
    mut ships: ShopShips,
) {
    let owner = raw_id(event.client_id, &clients);
    purchase(
        Purchase::BuyOutfit(&event.event().outfit),
        owner,
        event.client_id,
        &mut commands,
        &content.0,
        &storage,
        &mut ships,
    );
}

pub(super) fn on_sell_outfit(
    event: On<FromClient<SellOutfit>>,
    mut commands: Commands,
    clients: Query<&NetworkId>,
    content: Res<Content>,
    storage: Res<StorageResource>,
    mut ships: ShopShips,
) {
    let owner = raw_id(event.client_id, &clients);
    purchase(
        Purchase::SellOutfit(&event.event().outfit),
        owner,
        event.client_id,
        &mut commands,
        &content.0,
        &storage,
        &mut ships,
    );
}

#[allow(clippy::too_many_arguments)]
fn purchase(
    action: Purchase<'_>,
    owner: u64,
    client: ClientId,
    commands: &mut Commands,
    pack: &ContentPack,
    storage: &StorageResource,
    ships: &mut ShopShips,
) {
    let Some((
        _,
        docked,
        mut ship,
        mut spec,
        mut info,
        mut equipped,
        mut banks,
        mut hull,
        mut fuel,
        mut wallet,
        mut account,
        name,
        quests,
        reputation,
    )) = ships.iter_mut().find(|(o, ..)| o.0 == owner)
    else {
        notice(commands, client, "Dock at a port before using its shops.");
        return;
    };
    if ship.0.is_destroyed() {
        return;
    }
    let candidate = (|| -> Result<_, String> {
        let port = pack
            .dockable(&docked.system, &docked.location)
            .ok_or("This port is unavailable.")?;
        let mut ids = equipped.ids.clone();
        let mut hull_id = info.def_id.clone();
        let mut credits = wallet.0.credits;
        let mut replacement = false;
        match action {
            Purchase::Ship(id) => {
                if !port
                    .shipyard()
                    .is_some_and(|yard| yard.ships.iter().any(|item| item == id))
                {
                    return Err("This shipyard does not stock that hull.".into());
                }
                if id == hull_id {
                    return Err("You already own that hull.".into());
                }
                let old = pack.ship(&hull_id).ok_or("Current hull is unavailable.")?;
                let new = pack.ship(id).ok_or("Requested hull is unavailable.")?;
                let cargo: u64 = wallet.0.cargo.iter().map(|(_, qty)| u64::from(*qty)).sum();
                if cargo > u64::from(new.cargo) {
                    return Err("The new hull cannot carry your cargo.".into());
                }
                let balance = (u64::from(credits) + u64::from(old.price / 2))
                    .checked_sub(u64::from(new.price))
                    .ok_or("You cannot afford that hull after trade-in.")?;
                credits = u32::try_from(balance).map_err(|_| "Credit balance overflow.")?;
                hull_id = id.to_owned();
                replacement = true;
            }
            Purchase::BuyOutfit(id) => {
                if !port
                    .outfitter()
                    .is_some_and(|shop| shop.outfits.iter().any(|item| item == id))
                {
                    return Err("This outfitter does not stock that equipment.".into());
                }
                let outfit = pack.outfit(id).ok_or("Unknown outfit.")?;
                credits = credits
                    .checked_sub(outfit.price)
                    .ok_or("You cannot afford that outfit.")?;
                ids.push(id.to_owned());
            }
            Purchase::SellOutfit(id) => {
                if port.outfitter().is_none() {
                    return Err("There is no outfitter at this port.".into());
                }
                let index = ids
                    .iter()
                    .position(|item| item == id)
                    .ok_or("That outfit is not installed.")?;
                let outfit = pack.outfit(id).ok_or("Unknown outfit.")?;
                credits = credits
                    .checked_add(outfit.price / 2)
                    .ok_or("Credit balance overflow.")?;
                ids.remove(index);
            }
        }
        let stats = pack.loadout_stats(&hull_id, &ids)?;
        let def = pack.ship(&hull_id).ok_or("Owned hull is unavailable.")?;
        let mut state = ship.0.clone();
        state.max_shields = stats.max_shields;
        state.max_armor = stats.max_armor;
        state.max_fuel = stats.max_fuel;
        if replacement {
            state.shields = stats.max_shields;
            state.armor = stats.max_armor;
            state.fuel = stats.max_fuel;
        } else {
            state.shields = state.shields.min(stats.max_shields);
            state.armor = state.armor.min(stats.max_armor);
            state.fuel = state.fuel.min(stats.max_fuel);
        }
        let saved = crate::PlayerState {
            credits,
            cargo: wallet.0.cargo.clone(),
            ship_id: hull_id.clone(),
            system: docked.system.clone(),
            position: (state.position.x, state.position.y),
            fuel: state.fuel,
            shields: state.shields,
            armor: state.armor,
            active_quests_json: encode_active_quests(quests),
            equipped_outfits: Some(ids.clone()),
            reputation_json: reputation
                .map(super::factions::encode_reputation)
                .unwrap_or_else(|| "[]".to_owned()),
        };
        if let Some(db) = storage.storage.as_ref() {
            let name = name.ok_or("Cannot save this player's purchase.")?;
            db.save_player(&name.0, &saved)
                .map_err(|_| "Purchase cancelled: storage could not save it.")?;
        }
        Ok((hull_id, ids, credits, state, def))
    })();
    match candidate {
        Err(error) => notice(commands, client, error),
        Ok((hull_id, ids, credits, state, def)) => {
            // Mutate authoritative components directly so a second intent in the
            // same update sees the completed purchase, not a deferred snapshot.
            wallet.0.credits = credits;
            account.credits = credits;
            account.cargo_capacity = def.cargo;
            spec.cargo_capacity = def.cargo;
            spec.flight = FlightModel {
                turn_rate: def.turn_rate,
                acceleration: def.acceleration,
                max_speed: def.max_speed,
                drag: DRAG,
            };
            info.def_id = hull_id;
            info.name.clone_from(&def.name);
            *hull = Hull {
                shields: state.shields,
                max_shields: state.max_shields,
                armor: state.armor,
                max_armor: state.max_armor,
            };
            *fuel = Fuel {
                current: state.fuel,
                max: state.max_fuel,
            };
            *banks = weapon_banks(pack, &ids);
            equipped.ids = ids;
            ship.0 = state;
            notice(
                commands,
                client,
                format!("Purchase complete. {} cr remaining.", credits),
            );
        }
    }
}
