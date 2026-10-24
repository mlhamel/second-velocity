use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use evr_data::{ContentPack, OutfitKind};
use evr_protocol::{
    AccountState, BuyCommodity, BuyOutfit, BuyShip, DockableTag, DockedAt, EquippedOutfits,
    PortMarket, SellCommodity, SellOutfit, ShipInfo,
};

use crate::{MyNetworkId, chief::ChiefPanelState, input::ClientContent};

const CYAN: Color = Color::srgb(0.55, 1.0, 0.85);
const DIM: Color = Color::srgb(0.42, 0.44, 0.5);
const BODY: Color = Color::srgb(0.92, 0.93, 0.96);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ShopTab {
    #[default]
    Market,
    Shipyard,
    Outfitter,
}

impl ShopTab {
    const ALL: [Self; 3] = [Self::Market, Self::Shipyard, Self::Outfitter];

    fn index(self) -> usize {
        match self {
            Self::Market => 0,
            Self::Shipyard => 1,
            Self::Outfitter => 2,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Market => "Market",
            Self::Shipyard => "Shipyard",
            Self::Outfitter => "Outfitter",
        }
    }
}

#[derive(Resource, Default)]
struct ShopState {
    location: Option<(String, String)>,
    tab: ShopTab,
}

impl ShopState {
    fn sync(&mut self, docked: Option<&DockedAt>, available: [bool; 3]) {
        let location = docked.map(|d| (d.system.clone(), d.location.clone()));
        if self.location != location || !available[self.tab.index()] {
            self.tab = ShopTab::Market;
        }
        self.location = location;
    }

    fn cycle(&mut self, available: [bool; 3], reverse: bool) {
        let step = if reverse { 2 } else { 1 };
        for offset in 1..=3 {
            let tab = ShopTab::ALL[(self.tab.index() + offset * step) % 3];
            if available[tab.index()] {
                self.tab = tab;
                break;
            }
        }
    }
}

#[derive(Component)]
struct ShopPanel;

#[derive(Component, Debug, Clone, PartialEq, Eq)]
enum ShopAction {
    Tab(ShopTab),
    BuyCommodity(String),
    SellCommodity(String),
    BuyShip(String),
    BuyOutfit(String),
    SellOutfit(String),
}

#[derive(Debug, Clone, PartialEq)]
enum RowIcon {
    Hidden,
    Fallback,
    Asset(String),
}

impl From<Option<String>> for RowIcon {
    fn from(icon: Option<String>) -> Self {
        match icon {
            Some(path) => Self::Asset(path),
            None => Self::Fallback,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ShopRow {
    icon: RowIcon,
    text: String,
    buttons: Vec<(String, Option<ShopAction>)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct ShopModel {
    heading: String,
    available: [bool; 3],
    tab: ShopTab,
    blocked: bool,
    rows: Vec<ShopRow>,
    keyboard: Vec<(Option<ShopAction>, Option<ShopAction>)>,
}

impl ShopModel {
    fn row(&mut self, text: String, buttons: Vec<(String, Option<ShopAction>)>) {
        self.rows.push(ShopRow {
            icon: RowIcon::Hidden,
            text,
            buttons,
        });
    }

    fn item(&mut self, icon: RowIcon, text: String, buttons: Vec<(String, Option<ShopAction>)>) {
        self.rows.push(ShopRow {
            icon,
            text,
            buttons,
        });
    }

    fn permits(&self, action: &ShopAction) -> bool {
        if self.blocked {
            return false;
        }
        match action {
            ShopAction::Tab(tab) => self.available[tab.index()],
            _ => self
                .rows
                .iter()
                .flat_map(|row| &row.buttons)
                .any(|(_, candidate)| candidate.as_ref() == Some(action)),
        }
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ShopSystems;

pub struct ShopsPlugin;

impl Plugin for ShopsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShopState>().add_systems(
            Update,
            (update_shops, scroll_shops).chain().in_set(ShopSystems),
        );
    }
}

pub(crate) fn spawn_panel(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        ShopPanel,
        Node {
            display: Display::None,
            width: Val::Px(800.0),
            max_height: Val::Px(430.0),
            flex_shrink: 0.0,
            overflow: Overflow::scroll_y(),
            padding: UiRect::all(Val::Px(10.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(5.0),
            ..default()
        },
        ScrollPosition::default(),
        BackgroundColor(Color::srgba(0.03, 0.05, 0.1, 0.9)),
    ));
}

fn net_hull_price(price: u32, old_price: u32) -> i64 {
    i64::from(price) - i64::from(old_price / 2)
}

fn outfit_effect(kind: &OutfitKind) -> String {
    match kind {
        OutfitKind::Weapon(weapon) => format!(
            "weapon: {:.0} damage / {:.2}s, speed {:.0}, range {:.0}; 1 slot",
            weapon.damage,
            weapon.cooldown,
            weapon.projectile_speed,
            weapon.projectile_speed * weapon.projectile_ttl,
        ),
        OutfitKind::ShieldGenerator { max_shields } => {
            format!("+{max_shields:.0} shield capacity; one generator per hull")
        }
        OutfitKind::FuelTank { fuel } => {
            format!("+{fuel:.0} fuel capacity; one tank per hull")
        }
        OutfitKind::ArmorPlate { armor } => {
            format!("+{armor:.0} armor capacity; one plate per hull")
        }
    }
}

fn equipped_counts(ids: &[String]) -> Vec<(&str, usize)> {
    let mut counts = Vec::new();
    for id in ids {
        if let Some((_, count)) = counts.iter_mut().find(|(key, _)| *key == id.as_str()) {
            *count += 1;
        } else {
            counts.push((id.as_str(), 1));
        }
    }
    counts
}

#[allow(clippy::too_many_arguments)]
fn shop_model(
    pack: &ContentPack,
    info: &ShipInfo,
    account: &AccountState,
    equipped: Option<&EquippedOutfits>,
    docked: &DockedAt,
    tab: ShopTab,
    blocked: bool,
    prices: Option<&PortMarket>,
) -> ShopModel {
    let mut model = ShopModel {
        heading: format!("docked at {}", docked.location),
        tab,
        blocked,
        ..default()
    };
    let Some(port) = pack.dockable(&docked.system, &docked.location) else {
        return model;
    };
    model.available = [true, port.shipyard().is_some(), port.outfitter().is_some()];
    if blocked {
        model
            .heading
            .push_str(" — Esc dismisses the encounter to use shops");
    } else {
        model.heading.push_str(" — Tab / Shift+Tab change service");
    }
    let ids = equipped
        .map(|outfits| outfits.ids.as_slice())
        .unwrap_or(&[]);
    let owned = pack.ship(&info.def_id);
    if tab != ShopTab::Market {
        let hull_name = owned.map_or(info.def_id.as_str(), |ship| ship.name.as_str());
        let summary = match (equipped, pack.loadout_stats(&info.def_id, ids), owned) {
            (Some(_), Ok(stats), Some(ship)) => format!(
                "{hull_name} — shields {:.0}  armor {:.0}  fuel {:.0}  weapons {}/{} ({} free)",
                stats.max_shields,
                stats.max_armor,
                stats.max_fuel,
                stats.weapon_slots_used,
                ship.weapon_slots,
                ship.weapon_slots.saturating_sub(stats.weapon_slots_used),
            ),
            (None, _, _) => format!("{hull_name} — awaiting equipped loadout"),
            (_, Err(error), _) => format!("{hull_name} — {error}"),
            _ => hull_name.to_owned(),
        };
        model.row(summary, vec![]);
    }
    match tab {
        ShopTab::Market => {
            for (index, commodity) in port.commodities().iter().enumerate() {
                let held = account
                    .cargo
                    .iter()
                    .find(|(id, _)| id == &commodity.id)
                    .map_or(0, |(_, qty)| *qty);
                let name = pack
                    .commodity(&commodity.id)
                    .map_or(commodity.id.as_str(), |def| def.name.as_str());
                let price = prices
                    .and_then(|market| {
                        market
                            .prices
                            .iter()
                            .find(|entry| entry.commodity == commodity.id)
                            .map(|entry| entry.price)
                    })
                    .unwrap_or(commodity.price);
                let buy = ShopAction::BuyCommodity(commodity.id.clone());
                let sell = ShopAction::SellCommodity(commodity.id.clone());
                model.item(
                    pack.commodity(&commodity.id)
                        .and_then(|def| def.icon.clone())
                        .into(),
                    format!("{}  {name} @ {price} cr (hold {held})", index + 1,),
                    vec![
                        ("Buy 1".into(), Some(buy.clone())),
                        ("Sell 1".into(), Some(sell.clone())),
                    ],
                );
                model.keyboard.push((Some(buy), Some(sell)));
            }
            model.row("1–5 buy one commodity; Shift+1–5 sell one".into(), vec![]);
        }
        ShopTab::Shipyard => {
            let trade_in = owned.map_or(0, |ship| ship.price / 2);
            model.row(
                format!("Hull trade-in: {trade_in} cr (50% base). Installed outfits transfer."),
                vec![],
            );
            for (index, id) in port
                .shipyard()
                .into_iter()
                .flat_map(|yard| &yard.ships)
                .enumerate()
            {
                let Some(ship) = pack.ship(id) else {
                    continue;
                };
                let net = net_hull_price(ship.price, owned.map_or(0, |old| old.price));
                let cargo: u64 = account.cargo.iter().map(|(_, qty)| u64::from(*qty)).sum();
                let fit = pack.loadout_stats(id, ids);
                let reason = if id == &info.def_id {
                    Some("current hull".to_owned())
                } else if equipped.is_none() {
                    Some("awaiting loadout".to_owned())
                } else if cargo > u64::from(ship.cargo) {
                    Some("cargo will not fit".to_owned())
                } else if let Err(error) = &fit {
                    Some(error.clone())
                } else if net > i64::from(account.credits) {
                    Some("insufficient credits".to_owned())
                } else {
                    None
                };
                let stats = fit.map_or_else(
                    |_| {
                        format!(
                            "base shields {:.0}  armor {:.0}  fuel {}",
                            ship.max_shields, ship.max_armor, ship.fuel
                        )
                    },
                    |stats| {
                        format!(
                            "equipped shields {:.0}  armor {:.0}  fuel {:.0}",
                            stats.max_shields, stats.max_armor, stats.max_fuel
                        )
                    },
                );
                let price = if net < 0 {
                    format!("{} cr back", -net)
                } else {
                    format!("{net} cr net")
                };
                let action = reason.is_none().then(|| ShopAction::BuyShip(id.clone()));
                model.item(
                    ship.sprite.clone().into(),
                    format!("{}  {} — {} cr list / {price}\n{stats}  cargo {}  slots {}  speed {:.0}  accel {:.0}  turn {:.1}\n{}{}",
                        index + 1, ship.name, ship.price, ship.cargo, ship.weapon_slots,
                        ship.max_speed, ship.acceleration, ship.turn_rate, ship.description,
                        reason.as_ref().map(|reason| format!(" — {reason}")).unwrap_or_default()),
                    vec![("Buy hull".into(), action.clone())],
                );
                model.keyboard.push((action, None));
            }
            model.row(
                "1–9 buy hull. Net price includes your hull trade-in; no free factory outfits."
                    .into(),
                vec![],
            );
        }
        ShopTab::Outfitter => {
            model.row(
                "Equipped (sell one copy for 50% list price):".into(),
                vec![],
            );
            for (id, count) in equipped_counts(ids) {
                let (name, detail, resale) = pack.outfit(id).map_or_else(
                    || (id.to_owned(), "unknown outfit".into(), 0),
                    |outfit| {
                        (
                            outfit.name.clone(),
                            outfit_effect(&outfit.kind),
                            outfit.price / 2,
                        )
                    },
                );
                model.item(
                    pack.outfit(id)
                        .and_then(|outfit| outfit.icon.clone())
                        .into(),
                    format!("{name} ×{count} — {detail}"),
                    vec![(
                        format!("Sell 1 +{resale} cr"),
                        Some(ShopAction::SellOutfit(id.to_owned())),
                    )],
                );
            }
            if ids.is_empty() && equipped.is_some() {
                model.row("No outfits equipped.".into(), vec![]);
            }
            model.row("Outfitter stock:".into(), vec![]);
            for (index, id) in port
                .outfitter()
                .into_iter()
                .flat_map(|shop| &shop.outfits)
                .enumerate()
            {
                let Some(outfit) = pack.outfit(id) else {
                    continue;
                };
                let count = ids.iter().filter(|equipped_id| *equipped_id == id).count();
                let mut proposed = ids.to_vec();
                proposed.push(id.clone());
                let reason = if equipped.is_none() {
                    Some("awaiting loadout".to_owned())
                } else if account.credits < outfit.price {
                    Some("insufficient credits".to_owned())
                } else {
                    pack.loadout_stats(&info.def_id, &proposed).err()
                };
                let buy = reason.is_none().then(|| ShopAction::BuyOutfit(id.clone()));
                let sell = (count > 0).then(|| ShopAction::SellOutfit(id.clone()));
                model.item(
                    outfit.icon.clone().into(),
                    format!(
                        "{}  {} — {} cr / sell {} cr (equipped {count})\n{} — {}{}",
                        index + 1,
                        outfit.name,
                        outfit.price,
                        outfit.price / 2,
                        outfit_effect(&outfit.kind),
                        outfit.description,
                        reason
                            .as_ref()
                            .map(|reason| format!(" — {reason}"))
                            .unwrap_or_default()
                    ),
                    vec![
                        ("Buy + install".into(), buy.clone()),
                        ("Sell 1".into(), sell.clone()),
                    ],
                );
                model.keyboard.push((buy, sell));
            }
            model.row("1–9 buy + install; Shift+1–9 sell one stock outfit. Server validates all purchases.".into(), vec![]);
        }
    }
    model
}

const DIGITS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

fn keyboard_action(model: &ShopModel, keys: &ButtonInput<KeyCode>) -> Option<ShopAction> {
    if model.blocked
        || keys.any_just_pressed([KeyCode::Tab, KeyCode::KeyU, KeyCode::Escape, KeyCode::KeyT])
    {
        return None;
    }
    let limit = if model.tab == ShopTab::Market { 5 } else { 9 };
    DIGITS
        .iter()
        .take(limit)
        .enumerate()
        .find_map(|(index, key)| {
            if !keys.just_pressed(*key) {
                return None;
            }
            let (buy, sell) = model.keyboard.get(index)?;
            if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
                sell.clone()
            } else {
                buy.clone()
            }
        })
}

fn send_action(commands: &mut Commands, action: ShopAction) {
    match action {
        ShopAction::BuyCommodity(commodity) => {
            commands.client_trigger(BuyCommodity { commodity, qty: 1 })
        }
        ShopAction::SellCommodity(commodity) => {
            commands.client_trigger(SellCommodity { commodity, qty: 1 })
        }
        ShopAction::BuyShip(ship) => commands.client_trigger(BuyShip { ship }),
        ShopAction::BuyOutfit(outfit) => commands.client_trigger(BuyOutfit { outfit }),
        ShopAction::SellOutfit(outfit) => commands.client_trigger(SellOutfit { outfit }),
        ShopAction::Tab(_) => {}
    }
}

type ShopShips<'w, 's> = Query<
    'w,
    's,
    (
        &'static ShipInfo,
        &'static AccountState,
        Option<&'static EquippedOutfits>,
        Option<&'static DockedAt>,
    ),
>;

#[allow(clippy::too_many_arguments)]
fn update_shops(
    mut commands: Commands,
    my_id: Res<MyNetworkId>,
    content: Res<ClientContent>,
    asset_server: Res<AssetServer>,
    chief: Res<ChiefPanelState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<ShopState>,
    ships: ShopShips,
    ports: Query<(&DockableTag, &PortMarket)>,
    mut panel: Query<(Entity, &mut Node, &mut ScrollPosition), With<ShopPanel>>,
    interactions: Query<(&Interaction, &ShopAction), Changed<Interaction>>,
    mut previous: Local<Option<ShopModel>>,
) {
    let Ok((root, mut node, mut scroll)) = panel.single_mut() else {
        return;
    };
    let player = ships
        .iter()
        .find(|(info, _, _, _)| info.owner == Some(my_id.0));
    let Some((info, account, equipped, Some(docked))) = player else {
        state.sync(None, [true, false, false]);
        node.display = Display::None;
        *previous = None;
        return;
    };
    node.display = Display::Flex;
    let available = content
        .0
        .dockable(&docked.system, &docked.location)
        .map_or([true, false, false], |port| {
            [true, port.shipyard().is_some(), port.outfitter().is_some()]
        });
    let old_location = state.location.clone();
    state.sync(Some(docked), available);
    let location_changed = old_location != state.location;
    let blocked = chief.visible;
    let market = ports
        .iter()
        .find(|(tag, _)| tag.system == docked.system && tag.id == docked.location)
        .map(|(_, prices)| prices);
    let mut model = shop_model(
        &content.0, info, account, equipped, docked, state.tab, blocked, market,
    );
    // Never route an old port's click or a held transition key to a new port.
    if !blocked
        && !location_changed
        && !keys.any_just_pressed([KeyCode::KeyU, KeyCode::Escape, KeyCode::KeyT])
    {
        let clicked = interactions
            .iter()
            .find(|(interaction, action)| {
                **interaction == Interaction::Pressed && model.permits(action)
            })
            .map(|(_, action)| action.clone());
        if keys.just_pressed(KeyCode::Tab) {
            state.cycle(
                available,
                keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]),
            );
        } else if let Some(ShopAction::Tab(tab)) = clicked {
            state.tab = tab;
        } else if let Some(action) = clicked.or_else(|| keyboard_action(&model, &keys)) {
            send_action(&mut commands, action);
        }
        if model.tab != state.tab {
            model = shop_model(
                &content.0, info, account, equipped, docked, state.tab, blocked, market,
            );
            scroll.0 = Vec2::ZERO;
        }
    }
    if location_changed {
        scroll.0 = Vec2::ZERO;
    }
    if previous.as_ref() == Some(&model) {
        return;
    }
    commands
        .entity(root)
        .despawn_children()
        .with_children(|parent| render_panel(parent, &model, &asset_server));
    *previous = Some(model);
}

fn scroll_shops(
    mut wheel: MessageReader<MouseWheel>,
    keys: Res<ButtonInput<KeyCode>>,
    chief: Res<ChiefPanelState>,
    mut panel: Query<(&Node, &ComputedNode, &mut ScrollPosition), With<ShopPanel>>,
) {
    let mut delta = 0.0;
    for event in wheel.read() {
        delta -= event.y
            * if event.unit == MouseScrollUnit::Line {
                28.0
            } else {
                1.0
            };
    }
    if keys.just_pressed(KeyCode::PageDown) {
        delta += 260.0;
    }
    if keys.just_pressed(KeyCode::PageUp) {
        delta -= 260.0;
    }
    let Ok((node, computed, mut scroll)) = panel.single_mut() else {
        return;
    };
    if node.display == Display::None || chief.visible {
        return;
    }
    let max = ((computed.content_size().y - computed.size().y) * computed.inverse_scale_factor())
        .max(0.0);
    scroll.y = (scroll.y + delta).clamp(0.0, max);
}
fn text(parent: &mut ChildSpawnerCommands, value: &str, color: Color) {
    parent.spawn((
        Text(value.to_owned()),
        TextFont {
            font_size: bevy::text::FontSize::Px(13.0),
            ..default()
        },
        TextColor(color),
        Node {
            flex_shrink: 0.0,
            ..default()
        },
    ));
}

fn button(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    action: Option<ShopAction>,
    selected: bool,
) {
    let enabled = action.is_some();
    let mut entity = parent.spawn((
        Button,
        Node {
            padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(if selected {
            Color::srgb(0.1, 0.25, 0.25)
        } else {
            Color::srgb(0.08, 0.11, 0.16)
        }),
    ));
    if let Some(action) = action {
        entity.insert(action);
    }
    entity.with_children(|parent| text(parent, label, if enabled { CYAN } else { DIM }));
}

const SLOT: Color = Color::srgb(0.16, 0.19, 0.24);

fn icon_square(parent: &mut ChildSpawnerCommands, icon: &RowIcon, assets: &AssetServer) {
    let mut entity = parent.spawn((
        Node {
            width: Val::Px(20.0),
            height: Val::Px(20.0),
            flex_shrink: 0.0,
            ..default()
        },
        BackgroundColor(SLOT),
    ));
    if let RowIcon::Asset(path) = icon {
        entity.insert(ImageNode::new(assets.load(path.clone())));
    }
}

fn row_contents(parent: &mut ChildSpawnerCommands, row: &ShopRow, blocked: bool) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|parent| {
            text(parent, &row.text, BODY);
            if !row.buttons.is_empty() {
                parent
                    .spawn(Node {
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .with_children(|buttons| {
                        for (label, action) in &row.buttons {
                            button(
                                buttons,
                                label,
                                if blocked { None } else { action.clone() },
                                false,
                            );
                        }
                    });
            }
        });
}

fn render_panel(parent: &mut ChildSpawnerCommands, model: &ShopModel, assets: &AssetServer) {
    text(parent, &model.heading, CYAN);
    parent
        .spawn(Node {
            column_gap: Val::Px(8.0),
            flex_shrink: 0.0,
            ..default()
        })
        .with_children(|tabs| {
            for tab in ShopTab::ALL {
                let available = model.available[tab.index()];
                let label = if available {
                    tab.name().to_owned()
                } else {
                    format!("{} — unavailable", tab.name())
                };
                button(
                    tabs,
                    &label,
                    (available && !model.blocked).then_some(ShopAction::Tab(tab)),
                    model.tab == tab,
                );
            }
        });
    for row in &model.rows {
        if row.icon == RowIcon::Hidden {
            row_contents(parent, row, model.blocked);
            continue;
        }
        parent
            .spawn(Node {
                column_gap: Val::Px(8.0),
                align_items: AlignItems::FlexStart,
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|parent| {
                icon_square(parent, &row.icon, assets);
                row_contents(parent, row, model.blocked);
            });
    }
    text(parent, "Mouse wheel / PgUp / PgDn — scroll", DIM);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(tab: ShopTab) -> ShopModel {
        ShopModel {
            tab,
            keyboard: vec![(
                Some(ShopAction::BuyCommodity("food".into())),
                Some(ShopAction::SellCommodity("food".into())),
            )],
            ..default()
        }
    }

    #[test]
    fn encounter_visibility_blocks_digits_even_with_shop_selected() {
        let mut model = model(ShopTab::Outfitter);
        model.blocked = true;
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::Digit1);
        assert_eq!(keyboard_action(&model, &keys), None);
        assert!(!model.permits(&ShopAction::Tab(ShopTab::Market)));
    }

    #[test]
    fn market_number_and_shift_controls_are_preserved() {
        let model = model(ShopTab::Market);
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::Digit1);
        assert_eq!(
            keyboard_action(&model, &keys),
            Some(ShopAction::BuyCommodity("food".into()))
        );
        keys.press(KeyCode::ShiftLeft);
        assert_eq!(
            keyboard_action(&model, &keys),
            Some(ShopAction::SellCommodity("food".into()))
        );
        keys.press(KeyCode::Tab);
        assert_eq!(keyboard_action(&model, &keys), None);
    }

    #[test]
    fn location_change_and_undock_reset_tab() {
        let mut state = ShopState {
            location: Some(("sol".into(), "earth".into())),
            tab: ShopTab::Shipyard,
        };
        let docked = DockedAt {
            system: "sol".into(),
            location: "station".into(),
            kind: evr_protocol::DockableKind::Station,
        };
        state.sync(Some(&docked), [true, true, true]);
        assert_eq!(state.tab, ShopTab::Market);
        state.tab = ShopTab::Outfitter;
        state.sync(None, [true, false, false]);
        assert_eq!(state.tab, ShopTab::Market);
        assert!(state.location.is_none());
    }

    #[test]
    fn tab_cycle_skips_absent_services() {
        let mut state = ShopState::default();
        state.cycle([true, false, true], false);
        assert_eq!(state.tab, ShopTab::Outfitter);
        state.cycle([true, false, true], true);
        assert_eq!(state.tab, ShopTab::Market);
        state.tab = ShopTab::Outfitter;
        state.sync(None, [true, false, false]);
        assert_eq!(state.tab, ShopTab::Market);
    }

    #[test]
    fn trade_in_uses_half_base_price_without_unsigned_underflow() {
        assert_eq!(net_hull_price(1000, 501), 750);
        assert_eq!(net_hull_price(100, 1000), -400);
        assert_eq!(net_hull_price(u32::MAX, 0), i64::from(u32::MAX));
    }

    #[test]
    fn equipped_copies_are_counted_without_merging_different_outfits() {
        let ids = vec!["laser".into(), "tank".into(), "laser".into()];
        assert_eq!(equipped_counts(&ids), vec![("laser", 2), ("tank", 1)]);
    }

    #[test]
    fn encounter_dismissal_and_undock_never_also_purchase() {
        let model = model(ShopTab::Market);
        for transition in [KeyCode::Escape, KeyCode::KeyT, KeyCode::KeyU] {
            let mut keys = ButtonInput::default();
            keys.press(transition);
            keys.press(KeyCode::Digit1);
            assert_eq!(keyboard_action(&model, &keys), None);
        }
    }

    fn fixture() -> (ContentPack, ShipInfo, AccountState, DockedAt) {
        let mut pack = ContentPack::load_dir(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content"),
        )
        .expect("fixture content");
        let ship = pack.ship("shuttle").expect("starter hull");
        let info = ShipInfo {
            def_id: ship.id.clone(),
            name: ship.name.clone(),
            kind: evr_protocol::ShipKind::Player,
            owner: Some(1),
        };
        let account = AccountState {
            credits: u32::MAX,
            cargo: vec![],
            cargo_capacity: ship.cargo,
        };
        let port = &mut pack.systems[0].planets[0];
        port.shipyard = Some(evr_data::ShipyardDef {
            ships: pack.ships.iter().map(|ship| ship.id.clone()).collect(),
        });
        port.outfitter = Some(evr_data::OutfitterDef {
            outfits: vec!["light-blaster".into()],
        });
        let docked = DockedAt {
            system: pack.systems[0].id.clone(),
            location: pack.systems[0].planets[0].id.clone(),
            kind: evr_protocol::DockableKind::Planet,
        };
        (pack, info, account, docked)
    }

    #[test]
    fn full_weapon_slots_disable_install_but_keep_equipped_sale() {
        let (pack, info, account, docked) = fixture();
        let equipped = EquippedOutfits {
            ids: vec![
                "light-blaster".into();
                pack.ship(&info.def_id).unwrap().weapon_slots as usize
            ],
        };
        let model = shop_model(
            &pack,
            &info,
            &account,
            Some(&equipped),
            &docked,
            ShopTab::Outfitter,
            false,
            None,
        );
        assert_eq!(model.keyboard[0].0, None);
        assert_eq!(
            model.keyboard[0].1,
            Some(ShopAction::SellOutfit("light-blaster".into()))
        );
        assert!(
            model
                .rows
                .iter()
                .any(|row| row.text.contains("No free weapon slots"))
        );
        assert!(model.rows.iter().any(|row| row.text.contains("(0 free)")));
    }

    #[test]
    fn missing_loadout_and_oversized_cargo_disable_hull_purchase() {
        let (pack, info, mut account, docked) = fixture();
        let missing = shop_model(
            &pack,
            &info,
            &account,
            None,
            &docked,
            ShopTab::Shipyard,
            false,
            None,
        );
        assert!(missing.keyboard.iter().all(|(buy, _)| buy.is_none()));
        account.cargo.push(("food".into(), u32::MAX));
        let equipped = EquippedOutfits::default();
        let full = shop_model(
            &pack,
            &info,
            &account,
            Some(&equipped),
            &docked,
            ShopTab::Shipyard,
            false,
            None,
        );
        assert!(full.keyboard.iter().all(|(buy, _)| buy.is_none()));
        assert!(
            full.rows
                .iter()
                .any(|row| row.text.contains("cargo will not fit"))
        );
    }

    #[test]
    fn unstocked_equipment_can_be_sold_and_missing_services_are_disabled() {
        let (mut pack, info, account, docked) = fixture();
        pack.systems[0].planets[0]
            .outfitter
            .as_mut()
            .unwrap()
            .outfits
            .clear();
        let equipped = EquippedOutfits {
            ids: vec!["light-blaster".into()],
        };
        let model = shop_model(
            &pack,
            &info,
            &account,
            Some(&equipped),
            &docked,
            ShopTab::Outfitter,
            false,
            None,
        );
        assert!(model.permits(&ShopAction::SellOutfit("light-blaster".into())));
        assert!(model.keyboard.is_empty());
        pack.systems[0].planets[0].outfitter = None;
        pack.systems[0].planets[0].shipyard = None;
        let model = shop_model(
            &pack,
            &info,
            &account,
            Some(&equipped),
            &docked,
            ShopTab::Market,
            false,
            None,
        );
        assert_eq!(model.available, [true, false, false]);
        assert!(!model.permits(&ShopAction::Tab(ShopTab::Shipyard)));
        assert!(!model.permits(&ShopAction::Tab(ShopTab::Outfitter)));
    }
}
