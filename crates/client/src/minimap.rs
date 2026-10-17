use bevy::prelude::*;
use bevy_replicon::prelude::*;
use evr_protocol::{PlanetTag, Position, ShipInfo, ShipKind, SystemMembership};

use crate::MyNetworkId;
use crate::input::ClientContent;

const MINIMAP_SIZE: f32 = 220.0;
const MINIMAP_RANGE: f32 = 3000.0;
const DOT_SIZE: f32 = 6.0;
const DOT_COUNT: usize = 40;

#[derive(Component)]
struct MinimapRoot;

#[derive(Component)]
struct MinimapDot;

#[derive(Component)]
struct MinimapLabel;

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_minimap)
            .add_systems(Update, update_minimap);
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
        });
}

fn dot_color(planet: Option<&PlanetTag>, ship_info: Option<&ShipInfo>, my_id: u64) -> Color {
    if planet.is_some() {
        Color::srgb(0.35, 0.65, 1.0)
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
        Option<&'static PlanetTag>,
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

    for (pos, planet, ship_info) in &entities {
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

        assignments.push((mx, my, dot_color(planet, ship_info, my_id.0)));
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
