use bevy::prelude::*;
use bevy_replicon::prelude::*;
use evr_protocol::{
    BlackHoleTag, DockableKind, DockableTag, Heading, Position, ShipInfo, ShipKind, StarTag,
    SystemMembership,
};

use crate::MyNetworkId;
use crate::input::ClientContent;

const MINIMAP_SIZE: f32 = 220.0;
const MINIMAP_RANGE: f32 = 3000.0;
const DOT_SIZE: f32 = 6.0;
const DOT_COUNT: usize = 40;
const PLAYER_MARKER_COLOR: Color = Color::srgb(0.35, 1.0, 0.5);

#[derive(Component)]
struct MinimapRoot;

#[derive(Component)]
struct MinimapDot;

#[derive(Component)]
struct MinimapLabel;

#[derive(Component)]
struct MinimapPlayerMarker;

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_minimap)
            .add_systems(Update, update_minimap)
            .add_systems(Update, update_player_marker);
    }
}

fn spawn_minimap(mut commands: Commands) {
    commands
        .spawn((
            MinimapRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                top: Val::Px(16.0),
                width: Val::Px(MINIMAP_SIZE),
                height: Val::Px(MINIMAP_SIZE),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.05, 0.75)),
        ))
        .with_children(|root| {
            root.spawn((
                MinimapLabel,
                Text(String::new()),
                TextFont {
                    font_size: bevy::text::FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.75, 0.9)),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(6.0),
                    top: Val::Px(3.0),
                    ..default()
                },
            ));
            for _ in 0..DOT_COUNT {
                root.spawn((
                    MinimapDot,
                    Node {
                        position_type: PositionType::Absolute,
                        width: Val::Px(DOT_SIZE),
                        height: Val::Px(DOT_SIZE),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                ));
            }
            root.spawn((
                MinimapPlayerMarker,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(MINIMAP_SIZE / 2.0),
                    top: Val::Px(MINIMAP_SIZE / 2.0),
                    width: Val::Px(0.0),
                    height: Val::Px(0.0),
                    ..default()
                },
                Visibility::Hidden,
            ))
            .with_children(|marker| {
                marker.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(-2.0),
                        top: Val::Px(-2.0),
                        width: Val::Px(12.0),
                        height: Val::Px(4.0),
                        border_radius: BorderRadius::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(PLAYER_MARKER_COLOR),
                ));
                marker.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(-4.0),
                        top: Val::Px(-4.0),
                        width: Val::Px(8.0),
                        height: Val::Px(8.0),
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(PLAYER_MARKER_COLOR),
                    BorderColor::all(Color::srgba(0.0, 0.02, 0.08, 0.9)),
                ));
            });
        });
}

fn dot_color(
    planet: Option<&DockableTag>,
    star: Option<&StarTag>,
    black_hole: Option<&BlackHoleTag>,
    ship_info: Option<&ShipInfo>,
    my_id: u64,
) -> Color {
    if let Some(port) = planet {
        if port.kind == DockableKind::Station {
            Color::srgb(0.3, 1.0, 0.8)
        } else {
            Color::srgb(0.35, 0.65, 1.0)
        }
    } else if star.is_some() {
        Color::srgb(1.0, 0.6, 0.15)
    } else if black_hole.is_some() {
        Color::srgb(0.75, 0.3, 1.0)
    } else if let Some(info) = ship_info {
        match info.kind {
            ShipKind::Pirate => Color::srgb(1.0, 0.3, 0.2),
            ShipKind::Police => Color::srgb(0.3, 0.6, 1.0),
            ShipKind::Merchant => Color::srgb(1.0, 0.82, 0.38),
            ShipKind::Defender => Color::srgb(0.4, 0.85, 1.0),
            ShipKind::Explorer => Color::srgb(0.75, 0.55, 1.0),
            ShipKind::Player => {
                if info.owner == Some(my_id) {
                    Color::srgb(1.0, 1.0, 1.0)
                } else {
                    Color::srgb(0.7, 0.9, 0.7)
                }
            }
        }
    } else {
        Color::srgb(0.8, 0.7, 0.3)
    }
}

type MinimapEntities<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        Option<&'static DockableTag>,
        Option<&'static StarTag>,
        Option<&'static BlackHoleTag>,
        Option<&'static ShipInfo>,
    ),
    With<Remote>,
>;

fn update_minimap(
    my_id: Res<MyNetworkId>,
    mut roots: Query<&Children, With<MinimapRoot>>,
    mut dots: Query<(&mut Node, &mut BackgroundColor), With<MinimapDot>>,
    mut labels: Query<&mut Text, With<MinimapLabel>>,
    my_ships: Query<(&ShipInfo, &Position, &SystemMembership)>,
    entities: MinimapEntities,
    content: Res<ClientContent>,
) {
    let Ok(children) = roots.single_mut() else {
        return;
    };

    let Some((_, player_pos, membership)) = my_ships
        .iter()
        .find(|(info, _, _)| info.owner == Some(my_id.0))
    else {
        return;
    };

    let system_name = content.0.system(&membership.0).map(|sys| sys.name.clone());

    let half = MINIMAP_SIZE / 2.0;
    let scale = MINIMAP_SIZE / (MINIMAP_RANGE * 2.0);

    let mut assignments: Vec<(f32, f32, Color)> = Vec::with_capacity(DOT_COUNT);

    for (pos, planet, star, black_hole, ship_info) in &entities {
        if let Some(info) = ship_info
            && info.owner == Some(my_id.0)
        {
            continue;
        }

        let rel = pos.0 - player_pos.0;
        if rel.length() > MINIMAP_RANGE {
            continue;
        }

        let mx = half + rel.x * scale - DOT_SIZE / 2.0;
        let my = half - rel.y * scale - DOT_SIZE / 2.0;

        assignments.push((
            mx,
            my,
            dot_color(planet, star, black_hole, ship_info, my_id.0),
        ));
    }

    let mut i = 0;
    for child in children.iter() {
        if let Ok((mut node, mut bg)) = dots.get_mut(child) {
            if i < assignments.len() {
                let (mx, my, color) = assignments[i];
                node.left = Val::Px(mx);
                node.top = Val::Px(my);
                bg.0 = color;
                i += 1;
            } else {
                bg.0 = Color::NONE;
            }
        } else if let Ok(mut text) = labels.get_mut(child) {
            text.0 = system_name.clone().unwrap_or("Deep Space".to_owned());
        }
    }
}

fn update_player_marker(
    my_id: Res<MyNetworkId>,
    mut markers: Query<(&mut UiTransform, &mut Visibility), With<MinimapPlayerMarker>>,
    my_ships: Query<(&ShipInfo, &Heading)>,
) {
    let Ok((mut marker_transform, mut marker_visibility)) = markers.single_mut() else {
        return;
    };

    let Some((_, heading)) = my_ships
        .iter()
        .find(|(info, _)| info.owner == Some(my_id.0))
    else {
        *marker_visibility = Visibility::Hidden;
        return;
    };

    marker_transform.rotation = Rot2::radians(-heading.0);
    *marker_visibility = Visibility::Visible;
}
