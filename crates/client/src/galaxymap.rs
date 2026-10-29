use bevy::prelude::*;
use bevy_replicon::prelude::*;
use sv_protocol::{Position, ShipInfo, SystemControl};

use crate::MyNetworkId;
use crate::input::ClientContent;
use crate::menu::AppState;
use crate::ui::UiFont;

const MAP_WIDTH: f32 = 780.0;
const MAP_HEIGHT: f32 = 560.0;
const MAP_PAD: f32 = 48.0;
const DOT_SIZE: f32 = 10.0;
const METER_EPSILON: i32 = 1;
const TITLE_COLOR: Color = Color::srgb(0.6, 0.75, 0.9);
const PLAYER_MARKER_COLOR: Color = Color::srgb(0.35, 1.0, 0.5);
const NEUTRAL_DOT_COLOR: Color = Color::srgb(0.55, 0.55, 0.5);

#[derive(Component)]
struct GalaxyMapRoot;

#[derive(Component)]
struct GalaxyMapDot {
    system: String,
}

#[derive(Component)]
struct GalaxyMapSystemLabel;

#[derive(Component)]
struct GalaxyMapStandings;

#[derive(Component)]
struct GalaxyMapPlayerMarker;

pub struct GalaxyMapPlugin;

impl Plugin for GalaxyMapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn_galaxy_map)
            .add_systems(Update, toggle_galaxy_map.run_if(in_state(AppState::InGame)))
            .add_systems(Update, update_galaxy_map.run_if(in_state(AppState::InGame)));
    }
}

struct MapProjection {
    min: Vec2,
    span: Vec2,
}

impl MapProjection {
    fn from_content(pack: &sv_data::ContentPack) -> Self {
        let mut min = Vec2::splat(f32::MAX);
        let mut max = Vec2::splat(f32::MIN);
        for system in &pack.systems {
            let pos = Vec2::new(system.position.0, system.position.1);
            min = min.min(pos);
            max = max.max(pos);
        }
        let span = (max - min).max(Vec2::splat(1.0));
        Self { min, span }
    }

    fn project(&self, world: Vec2) -> Vec2 {
        let norm = (world - self.min) / self.span;
        Vec2::new(
            MAP_PAD + norm.x * (MAP_WIDTH - MAP_PAD * 2.0) - DOT_SIZE / 2.0,
            MAP_HEIGHT - MAP_PAD - norm.y * (MAP_HEIGHT - MAP_PAD * 2.0) - DOT_SIZE / 2.0,
        )
    }
}

fn spawn_galaxy_map(
    mut commands: Commands,
    content: Res<ClientContent>,
    ui_font: Res<UiFont>,
    existing: Query<(), With<GalaxyMapRoot>>,
) {
    if !existing.is_empty() {
        return;
    }
    let projection = MapProjection::from_content(&content.0);
    let mut map = commands.spawn((
        GalaxyMapRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.01, 0.05, 0.85)),
        Visibility::Hidden,
    ));
    map.with_children(|root| {
        root.spawn((
            Node {
                position_type: PositionType::Relative,
                width: Val::Px(MAP_WIDTH),
                height: Val::Px(MAP_HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.08, 0.9)),
            BorderColor::all(Color::srgba(0.3, 0.45, 0.7, 0.8)),
        ))
        .with_children(|panel| {
            panel.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(12.0),
                    top: Val::Px(8.0),
                    ..default()
                },
                Text("GALAXY - THE SCOREBOARD  [M]".to_owned()),
                TextFont {
                    font: bevy::text::FontSource::Handle(ui_font.0.clone()),
                    font_size: bevy::text::FontSize::Px(16.0),
                    ..default()
                },
                TextColor(TITLE_COLOR),
            ));
            for system in &content.0.systems {
                let pos = projection.project(Vec2::new(system.position.0, system.position.1));
                panel
                    .spawn((
                        GalaxyMapDot {
                            system: system.id.clone(),
                        },
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(pos.x),
                            top: Val::Px(pos.y),
                            width: Val::Px(DOT_SIZE),
                            height: Val::Px(DOT_SIZE),
                            border_radius: BorderRadius::all(Val::Px(DOT_SIZE / 2.0)),
                            ..default()
                        },
                        BackgroundColor(NEUTRAL_DOT_COLOR),
                    ))
                    .with_children(|dot| {
                        dot.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: Val::Px(DOT_SIZE + 4.0),
                                top: Val::Px(-2.0),
                                ..default()
                            },
                            GalaxyMapSystemLabel,
                            Text(system.name.clone()),
                            TextFont {
                                font: bevy::text::FontSource::Handle(ui_font.0.clone()),
                                font_size: bevy::text::FontSize::Px(8.0),
                                ..default()
                            },
                            TextColor(NEUTRAL_DOT_COLOR),
                        ));
                    });
            }
            panel.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(12.0),
                    bottom: Val::Px(10.0),
                    ..default()
                },
                GalaxyMapStandings,
                Text(String::new()),
                TextFont {
                    font: bevy::text::FontSource::Handle(ui_font.0.clone()),
                    font_size: bevy::text::FontSize::Px(8.0),
                    ..default()
                },
                TextColor(Color::srgb(0.75, 0.8, 0.85)),
            ));
            panel
                .spawn((
                    GalaxyMapPlayerMarker,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(0.0),
                        height: Val::Px(0.0),
                        ..default()
                    },
                ))
                .with_children(|marker| {
                    marker.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(-3.0),
                            top: Val::Px(-3.0),
                            width: Val::Px(10.0),
                            height: Val::Px(10.0),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(5.0)),
                            ..default()
                        },
                        BackgroundColor(PLAYER_MARKER_COLOR),
                        BorderColor::all(Color::srgba(0.0, 0.02, 0.08, 0.9)),
                    ));
                });
        });
    });
}

fn toggle_galaxy_map(
    keys: Res<ButtonInput<KeyCode>>,
    mut roots: Query<&mut Visibility, With<GalaxyMapRoot>>,
) {
    if !keys.just_pressed(KeyCode::KeyM) {
        return;
    }
    let Ok(mut visibility) = roots.single_mut() else {
        return;
    };
    *visibility = match *visibility {
        Visibility::Visible => Visibility::Hidden,
        _ => Visibility::Visible,
    };
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_galaxy_map(
    my_id: Res<MyNetworkId>,
    content: Res<ClientContent>,
    controls: Query<&SystemControl, With<Remote>>,
    my_ships: Query<(&ShipInfo, &Position, Option<&sv_protocol::PlayerReputation>)>,
    mut dots: Query<(&GalaxyMapDot, &mut BackgroundColor, &Children), With<GalaxyMapDot>>,
    mut labels: Query<
        (&mut Text, &mut TextColor),
        (With<GalaxyMapSystemLabel>, Without<GalaxyMapStandings>),
    >,
    mut standings: Query<&mut Text, (With<GalaxyMapStandings>, Without<GalaxyMapSystemLabel>)>,
    mut markers: Query<&mut Node, With<GalaxyMapPlayerMarker>>,
) {
    let Some((_, player_pos, reputation)) = my_ships
        .iter()
        .find(|(info, _, _)| info.owner == Some(my_id.0))
    else {
        return;
    };
    let projection = MapProjection::from_content(&content.0);

    for (dot, mut dot_bg, children) in dots.iter_mut() {
        let Some(system) = content.0.system(&dot.system) else {
            continue;
        };
        let control = controls.iter().find(|state| state.system == dot.system);
        let controller = control
            .map(|state| state.controller.as_str())
            .unwrap_or(system.government.as_str());
        let meter = control.map(|state| state.meter).unwrap_or(0);
        let color = content
            .0
            .faction(controller)
            .map(|faction| Color::srgb(faction.color.0, faction.color.1, faction.color.2))
            .unwrap_or(NEUTRAL_DOT_COLOR);
        dot_bg.0 = color;
        for child in children.iter() {
            if let Ok((mut text, mut text_color)) = labels.get_mut(child) {
                let mut label = system.name.clone();
                if meter.abs() >= METER_EPSILON {
                    label.push_str(&format!(" ({meter:+})"));
                }
                if text.0 != label {
                    text.0 = label;
                }
                text_color.0 = color;
            }
        }
    }

    if let Ok(mut standings) = standings.single_mut() {
        let mut rendered = String::from("Standing: ");
        for (index, faction) in content.0.factions.iter().enumerate() {
            let value = reputation
                .and_then(|rep| {
                    rep.standings
                        .iter()
                        .find(|(id, _)| id == &faction.id)
                        .map(|(_, value)| *value)
                })
                .unwrap_or(0);
            if index > 0 {
                rendered.push_str("   ");
            }
            rendered.push_str(&format!("{} {value:+}", faction.name));
        }
        if standings.0 != rendered {
            standings.0 = rendered;
        }
    }

    if let Ok(mut marker) = markers.single_mut() {
        let pos = projection.project(player_pos.0);
        marker.left = Val::Px(pos.x + DOT_SIZE / 2.0);
        marker.top = Val::Px(pos.y + DOT_SIZE / 2.0);
    }
}
