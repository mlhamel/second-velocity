use bevy::prelude::*;
use bevy_replicon::prelude::*;
use evr_data::ContentPack;
use evr_protocol::{
    BuyCommodity, ChiefEncounter, DockedAt, HailShip, PlanetTag, Position, RequestLand,
    RequestLiftOff, RespondToEncounter, SellCommodity, SetFiring, SetThrottle, SetTurn, SetWarp,
    ShipInfo, SystemMembership,
};
use evr_server::gameplay::LAND_RANGE;

use crate::MyNetworkId;
use crate::chief::ChiefPanelState;

#[derive(Resource, Default)]
struct InputState {
    throttle: bool,
    turn: i32,
    firing: bool,
    warp: bool,
}

#[derive(Resource)]
pub struct ClientContent(pub ContentPack);

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputState>()
            .add_systems(Update, poll_flight_input)
            .add_systems(Update, poll_action_input);
    }
}

fn poll_flight_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<InputState>,
    mut commands: Commands,
) {
    let throttle = keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]);
    if throttle != state.throttle {
        state.throttle = throttle;
        let val = if throttle { 1.0 } else { 0.0 };
        debug!("client: SetThrottle({val})");
        commands.client_trigger(SetThrottle(val));
    }

    let left = i32::from(keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]));
    let right = i32::from(keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]));
    let turn = right - left;
    if turn != state.turn {
        state.turn = turn;
        debug!("client: SetTurn({turn})");
        commands.client_trigger(SetTurn(turn as f32));
    }

    let firing = keys.pressed(KeyCode::Space);
    if firing != state.firing {
        state.firing = firing;
        debug!("client: SetFiring({firing})");
        commands.client_trigger(SetFiring(firing));
    }

    let warp = keys.pressed(KeyCode::KeyJ);
    if warp != state.warp {
        state.warp = warp;
        info!("client: SetWarp({warp})");
        commands.client_trigger(SetWarp(warp));
    }
}

const DIGIT_KEYS: [KeyCode; 5] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
];

#[allow(clippy::type_complexity)]
fn poll_action_input(
    my_id: Res<MyNetworkId>,
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    content: Res<ClientContent>,
    chief_panel: Option<Res<ChiefPanelState>>,
    ships: Query<(
        &ShipInfo,
        &Position,
        &SystemMembership,
        Option<&DockedAt>,
        Option<&ChiefEncounter>,
    )>,
    planets: Query<(&PlanetTag, &Position)>,
) {
    let Some((_, position, membership, docked, encounter)) = ships
        .iter()
        .find(|(info, _, _, _, _)| info.owner == Some(my_id.0))
    else {
        return;
    };

    if docked.is_none() && keys.just_pressed(KeyCode::KeyL) {
        let candidate = planets
            .iter()
            .filter(|(tag, _)| tag.system == membership.0)
            .filter(|(tag, planet_position)| {
                position.0.distance(planet_position.0) <= LAND_RANGE + tag.radius
            })
            .min_by_key(|(_, planet_position)| position.0.distance(planet_position.0).to_bits());
        if let Some((planet, _)) = candidate {
            info!("client: RequestLand({})", planet.id);
            commands.client_trigger(RequestLand {
                planet: planet.id.clone(),
            });
        }
    }

    if docked.is_some() && keys.just_pressed(KeyCode::KeyU) {
        info!("client: RequestLiftOff");
        commands.client_trigger(RequestLiftOff);
    }

    if docked.is_none() && keys.just_pressed(KeyCode::KeyH) {
        info!("client: HailShip");
        commands.client_trigger(HailShip);
    }

    let panel_open = chief_panel.is_some_and(|s| s.visible) && encounter.is_some();
    if panel_open && let Some(enc) = encounter {
        for (index, option) in enc.options.iter().enumerate() {
            if let Some(&digit) = DIGIT_KEYS.get(index)
                && keys.just_pressed(digit)
            {
                info!("client: RespondToEncounter({})", option.id);
                commands.client_trigger(RespondToEncounter {
                    choice: option.id.clone(),
                });
                return;
            }
        }
        return;
    }

    let Some(docked) = docked else {
        return;
    };

    let Some(system) = content.0.system(&docked.system) else {
        return;
    };
    let Some(planet) = system.planets.iter().find(|p| p.id == docked.planet) else {
        return;
    };
    for (index, commodity) in planet.commodities.iter().enumerate() {
        let Some(&digit) = DIGIT_KEYS.get(index) else {
            continue;
        };
        if keys.just_pressed(digit) {
            let selling = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
            if selling {
                info!("client: SellCommodity({}, 1)", commodity.id);
                commands.client_trigger(SellCommodity {
                    commodity: commodity.id.clone(),
                    qty: 1,
                });
            } else {
                info!("client: BuyCommodity({}, 1)", commodity.id);
                commands.client_trigger(BuyCommodity {
                    commodity: commodity.id.clone(),
                    qty: 1,
                });
            }
        }
    }
}
