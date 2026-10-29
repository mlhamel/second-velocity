use bevy::prelude::*;
use bevy_replicon::prelude::*;
use sv_data::ContentPack;
use sv_protocol::{
    ChiefEncounter, CycleTarget, DockableTag, DockedAt, HailShip, Position, RequestDock,
    RequestLiftOff, RespondToEncounter, SetAfterburner, SetBrake, SetFiring, SetSecondaryFiring,
    SetThrottle, SetTurn, SetWarp, ShipInfo, SystemMembership,
};
use sv_server::gameplay::LAND_RANGE;

use crate::MyNetworkId;
use crate::chief::ChiefPanelState;
use crate::menu::AppState;

#[derive(Resource, Default)]
struct InputState {
    throttle: bool,
    turn: i32,
    firing: bool,
    secondary_firing: bool,
    afterburner: bool,
    brake: bool,
    warp: bool,
    docked: Option<bool>,
}

#[derive(Debug, PartialEq)]
enum FlightChange {
    Throttle(f32),
    Turn(f32),
    Firing(bool),
    SecondaryFiring(bool),
    Afterburner(bool),
    Brake(bool),
    Warp(bool),
}

#[derive(Resource)]
pub struct ClientContent(pub ContentPack);

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputState>()
            .add_systems(Update, poll_flight_input.run_if(in_state(AppState::InGame)))
            .add_systems(Update, poll_action_input.run_if(in_state(AppState::InGame)));
    }
}

fn poll_flight_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<InputState>,
    mut commands: Commands,
    my_id: Res<MyNetworkId>,
    ships: Query<(&ShipInfo, Option<&DockedAt>)>,
) {
    let Some((_, docked)) = ships.iter().find(|(info, _)| info.owner == Some(my_id.0)) else {
        state.docked = None;
        return;
    };
    for change in flight_changes(&mut state, &keys, docked.is_some()) {
        match change {
            FlightChange::Throttle(value) => commands.client_trigger(SetThrottle(value)),
            FlightChange::Turn(value) => commands.client_trigger(SetTurn(value)),
            FlightChange::Firing(value) => commands.client_trigger(SetFiring(value)),
            FlightChange::SecondaryFiring(value) => {
                commands.client_trigger(SetSecondaryFiring(value))
            }
            FlightChange::Afterburner(value) => commands.client_trigger(SetAfterburner(value)),
            FlightChange::Brake(value) => commands.client_trigger(SetBrake(value)),
            FlightChange::Warp(value) => commands.client_trigger(SetWarp(value)),
        }
    }
}

fn flight_changes(
    state: &mut InputState,
    keys: &ButtonInput<KeyCode>,
    docked: bool,
) -> Vec<FlightChange> {
    let force = state.docked != Some(docked);
    state.docked = Some(docked);
    let mut changes = Vec::new();
    let throttle = !docked && keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]);
    if force || throttle != state.throttle {
        state.throttle = throttle;
        let val = if throttle { 1.0 } else { 0.0 };
        debug!("client: SetThrottle({val})");
        changes.push(FlightChange::Throttle(val));
    }

    let left = i32::from(keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]));
    let right = i32::from(keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]));
    let turn = if docked { 0 } else { right - left };
    if force || turn != state.turn {
        state.turn = turn;
        debug!("client: SetTurn({turn})");
        changes.push(FlightChange::Turn(turn as f32));
    }

    let firing = !docked && keys.pressed(KeyCode::Space);
    if force || firing != state.firing {
        state.firing = firing;
        debug!("client: SetFiring({firing})");
        changes.push(FlightChange::Firing(firing));
    }

    let secondary_firing = !docked && keys.pressed(KeyCode::KeyF);
    if force || secondary_firing != state.secondary_firing {
        state.secondary_firing = secondary_firing;
        debug!("client: SetSecondaryFiring({secondary_firing})");
        changes.push(FlightChange::SecondaryFiring(secondary_firing));
    }

    let afterburner = !docked && keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if force || afterburner != state.afterburner {
        state.afterburner = afterburner;
        debug!("client: SetAfterburner({afterburner})");
        changes.push(FlightChange::Afterburner(afterburner));
    }

    let brake = !docked && keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]);
    if force || brake != state.brake {
        state.brake = brake;
        debug!("client: SetBrake({brake})");
        changes.push(FlightChange::Brake(brake));
    }

    let warp = !docked && keys.pressed(KeyCode::KeyJ);
    if force || warp != state.warp {
        state.warp = warp;
        info!("client: SetWarp({warp})");
        changes.push(FlightChange::Warp(warp));
    }
    changes
}

const DIGIT_KEYS: [KeyCode; 5] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
];

#[allow(clippy::type_complexity)]
pub(crate) fn poll_action_input(
    my_id: Res<MyNetworkId>,
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    chief_panel: Option<Res<ChiefPanelState>>,
    ships: Query<(
        &ShipInfo,
        &Position,
        &SystemMembership,
        Option<&DockedAt>,
        Option<&ChiefEncounter>,
    )>,
    planets: Query<(&DockableTag, &Position)>,
) {
    let Some((_, position, _membership, docked, encounter)) = ships
        .iter()
        .find(|(info, _, _, _, _)| info.owner == Some(my_id.0))
    else {
        return;
    };

    if docked.is_none() && keys.just_pressed(KeyCode::KeyL) {
        let candidate = planets
            .iter()
            .filter(|(tag, planet_position)| {
                position.0.distance(planet_position.0) <= LAND_RANGE + tag.radius
            })
            .min_by_key(|(_, planet_position)| position.0.distance(planet_position.0).to_bits());
        if let Some((planet, _)) = candidate {
            info!("client: RequestDock({}/{})", planet.system, planet.id);
            commands.client_trigger(RequestDock {
                system: planet.system.clone(),
                location: planet.id.clone(),
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

    if docked.is_none() && encounter.is_none() && keys.just_pressed(KeyCode::KeyT) {
        info!("client: CycleTarget(hostile_only=false)");
        commands.client_trigger(CycleTarget {
            hostile_only: false,
        });
    }

    if docked.is_none() && keys.just_pressed(KeyCode::KeyR) {
        info!("client: CycleTarget(hostile_only=true)");
        commands.client_trigger(CycleTarget { hostile_only: true });
    }

    let panel_open = chief_panel.is_some_and(|s| s.visible);
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn held_controls_resume_after_docking_and_undocking() {
        let mut state = InputState::default();
        let mut keys = ButtonInput::default();
        for key in [KeyCode::KeyW, KeyCode::KeyA, KeyCode::Space, KeyCode::KeyJ] {
            keys.press(key);
        }
        let held = vec![
            FlightChange::Throttle(1.0),
            FlightChange::Turn(-1.0),
            FlightChange::Firing(true),
            FlightChange::SecondaryFiring(false),
            FlightChange::Afterburner(false),
            FlightChange::Brake(false),
            FlightChange::Warp(true),
        ];
        assert_eq!(flight_changes(&mut state, &keys, false), held);
        assert_eq!(
            flight_changes(&mut state, &keys, true),
            vec![
                FlightChange::Throttle(0.0),
                FlightChange::Turn(0.0),
                FlightChange::Firing(false),
                FlightChange::SecondaryFiring(false),
                FlightChange::Afterburner(false),
                FlightChange::Brake(false),
                FlightChange::Warp(false),
            ]
        );
        assert!(flight_changes(&mut state, &keys, true).is_empty());
        assert_eq!(
            flight_changes(&mut state, &keys, false),
            vec![
                FlightChange::Throttle(1.0),
                FlightChange::Turn(-1.0),
                FlightChange::Firing(true),
                FlightChange::SecondaryFiring(false),
                FlightChange::Afterburner(false),
                FlightChange::Brake(false),
                FlightChange::Warp(true),
            ]
        );
        assert!(flight_changes(&mut state, &keys, false).is_empty());
    }

    #[test]
    fn undocking_resends_false_states_after_keys_are_released_while_docked() {
        let mut state = InputState::default();
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::KeyW);
        flight_changes(&mut state, &keys, false);
        flight_changes(&mut state, &keys, true);
        keys.release(KeyCode::KeyW);
        assert!(flight_changes(&mut state, &keys, true).is_empty());
        assert_eq!(
            flight_changes(&mut state, &keys, false),
            vec![
                FlightChange::Throttle(0.0),
                FlightChange::Turn(0.0),
                FlightChange::Firing(false),
                FlightChange::SecondaryFiring(false),
                FlightChange::Afterburner(false),
                FlightChange::Brake(false),
                FlightChange::Warp(false),
            ]
        );
    }
}
