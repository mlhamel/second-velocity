use std::time::{Duration, Instant};

use bevy::prelude::*;
use sv_protocol::{
    AccountState, ActiveQuests, ChiefEncounter, DockableKind, DockableTag, DockedAt, Fuel, Hull,
    Notice, PlayerDied, Position, QuestTarget, ShipInfo, SystemMembership,
};
use sv_server::gameplay::LAND_RANGE;

use crate::MyNetworkId;
use crate::input::ClientContent;

const CYAN: Color = Color::srgb(0.55, 1.0, 0.85);
const AMBER: Color = Color::srgb(1.0, 0.85, 0.5);
const RED: Color = Color::srgb(1.0, 0.35, 0.3);
const VALUE: Color = Color::srgb(0.95, 0.95, 0.95);
const DIM: Color = Color::srgb(0.42, 0.44, 0.5);

const BAR_WIDTH: f32 = 170.0;

#[derive(Resource, Default)]
struct Noticeboard {
    text: String,
    at: Option<Instant>,
}

#[derive(Resource)]
struct HudLayout {
    system_line: Entity,
    stats_line: Entity,
    quest_line: Entity,
    prompt_line: Entity,
    notice_line: Entity,
    banner_line: Entity,
    shield_fill: Entity,
    armor_fill: Entity,
    fuel_fill: Entity,
}

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Noticeboard>()
            .add_systems(Startup, spawn_hud)
            .add_observer(on_notice)
            .add_observer(on_player_died)
            .add_systems(Update, update_hud);
    }
}

#[derive(Default)]
struct HudEntities {
    system_line: Option<Entity>,
    stats_line: Option<Entity>,
    quest_line: Option<Entity>,
    prompt_line: Option<Entity>,
    notice_line: Option<Entity>,
    banner_line: Option<Entity>,
    shield_fill: Option<Entity>,
    armor_fill: Option<Entity>,
    fuel_fill: Option<Entity>,
}

fn spawn_text(
    parent: &mut ChildSpawnerCommands,
    text: impl Into<String>,
    size: f32,
    color: Color,
) -> Entity {
    parent
        .spawn((
            Text(text.into()),
            TextFont {
                font_size: bevy::text::FontSize::Px(size),
                ..default()
            },
            TextColor(color),
        ))
        .id()
}

fn spawn_bar(parent: &mut ChildSpawnerCommands, label: &str, fill_color: Color) -> Entity {
    let mut fill = None;
    parent
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|bar| {
            spawn_text(bar, label.to_owned(), 13.0, DIM);
            bar.spawn((
                Node {
                    width: Val::Px(BAR_WIDTH),
                    height: Val::Px(9.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.08, 0.09, 0.13)),
            ))
            .with_children(|track| {
                fill = Some(
                    track
                        .spawn((
                            Node {
                                width: Val::Px(BAR_WIDTH),
                                height: Val::Px(9.0),
                                ..default()
                            },
                            BackgroundColor(fill_color),
                        ))
                        .id(),
                );
            });
        });
    fill.expect("bar fill must be spawned")
}

fn spawn_hud(mut commands: Commands) {
    let mut ids = HudEntities::default();

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(14.0),
            left: Val::Px(16.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|root| {
            ids.system_line = Some(spawn_text(root, "", 26.0, CYAN));
            ids.stats_line = Some(spawn_text(root, "", 16.0, VALUE));
            ids.shield_fill = Some(spawn_bar(root, "shield", Color::srgb(0.3, 0.8, 1.0)));
            ids.armor_fill = Some(spawn_bar(root, "armor", Color::srgb(0.95, 0.6, 0.3)));
            ids.fuel_fill = Some(spawn_bar(root, "fuel", Color::srgb(0.5, 0.9, 0.4)));
            ids.quest_line = Some(spawn_text(root, "", 14.0, AMBER));
            ids.prompt_line = Some(spawn_text(root, "", 15.0, CYAN));
            crate::shops::spawn_panel(root);
        });

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(16.0),
            left: Val::Percent(38.0),
            ..default()
        })
        .with_children(|parent| {
            ids.notice_line = Some(spawn_text(parent, "", 16.0, AMBER));
        });

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(42.0),
            left: Val::Percent(36.0),
            ..default()
        })
        .with_children(|parent| {
            ids.banner_line = Some(spawn_text(parent, "", 30.0, RED));
        });

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(12.0),
            left: Val::Px(16.0),
            ..default()
        })
        .with_children(|parent| {
            spawn_text(
                parent,
                "W thrust   A/D turn   Space fire   L land/dock   U undock   H hail   J warp   T host",
                13.0,
                DIM,
            );
        });

    commands.insert_resource(HudLayout {
        system_line: ids.system_line.expect("hud entities must exist"),
        stats_line: ids.stats_line.expect("hud entities must exist"),
        quest_line: ids.quest_line.expect("hud entities must exist"),
        prompt_line: ids.prompt_line.expect("hud entities must exist"),
        notice_line: ids.notice_line.expect("hud entities must exist"),
        banner_line: ids.banner_line.expect("hud entities must exist"),
        shield_fill: ids.shield_fill.expect("hud entities must exist"),
        armor_fill: ids.armor_fill.expect("hud entities must exist"),
        fuel_fill: ids.fuel_fill.expect("hud entities must exist"),
    });
}

fn on_notice(trigger: On<Notice>, mut board: ResMut<Noticeboard>) {
    info!("client: notice received: {}", trigger.event().text);
    board.text = trigger.event().text.clone();
    board.at = Some(Instant::now());
}

fn on_player_died(_death: On<PlayerDied>, mut board: ResMut<Noticeboard>) {
    info!("client: player ship destroyed");
    board.text = "Your ship was destroyed. The yard is building you another.".to_owned();
    board.at = Some(Instant::now());
}

type HudShips<'w, 's> = Query<
    'w,
    's,
    (
        &'static ShipInfo,
        &'static Hull,
        &'static Fuel,
        &'static AccountState,
        &'static SystemMembership,
        &'static Position,
        Option<&'static DockedAt>,
        Option<&'static ActiveQuests>,
        Option<&'static ChiefEncounter>,
    ),
>;

fn set_text(texts: &mut Query<&mut Text>, entity: Entity, value: String) {
    if let Ok(mut text) = texts.get_mut(entity) {
        text.0 = value;
    }
}

fn set_bar(nodes: &mut Query<&mut Node>, entity: Entity, fraction: f32) {
    if let Ok(mut node) = nodes.get_mut(entity) {
        node.width = Val::Px(BAR_WIDTH * fraction.clamp(0.0, 1.0));
    }
}

#[allow(clippy::too_many_arguments)]
fn update_hud(
    my_id: Res<MyNetworkId>,
    mut texts: Query<&mut Text>,
    mut nodes: Query<&mut Node>,
    layout: Res<HudLayout>,
    board: Res<Noticeboard>,
    content: Res<ClientContent>,
    ships: HudShips,
    planets: Query<(&DockableTag, &Position)>,
) {
    let my = ships
        .iter()
        .find(|(info, _, _, _, _, _, _, _, _)| info.owner == Some(my_id.0));

    let Some((_, hull, fuel, account, membership, position, docked, active_quests, encounter)) = my
    else {
        set_text(&mut texts, layout.system_line, String::new());
        set_text(&mut texts, layout.stats_line, String::new());
        set_text(&mut texts, layout.quest_line, String::new());
        set_text(&mut texts, layout.prompt_line, String::new());
        set_bar(&mut nodes, layout.shield_fill, 0.0);
        set_bar(&mut nodes, layout.armor_fill, 0.0);
        set_bar(&mut nodes, layout.fuel_fill, 0.0);
        set_text(&mut texts, layout.banner_line, "SHIP LOST".to_owned());
        set_notice(&mut texts, &layout, &board);
        return;
    };

    set_text(&mut texts, layout.banner_line, String::new());
    set_notice(&mut texts, &layout, &board);

    let system_name = content
        .0
        .system(&membership.0)
        .map(|system| system.name.as_str())
        .unwrap_or("Uncharted Space");
    set_text(&mut texts, layout.system_line, system_name.to_owned());

    let cargo_used: u32 = account.cargo.iter().map(|(_, qty)| qty).sum();
    set_text(
        &mut texts,
        layout.stats_line,
        format!(
            "{} cr   cargo {}/{}   fuel {:.0}/{}",
            account.credits, cargo_used, account.cargo_capacity, fuel.current, fuel.max
        ),
    );

    set_bar(
        &mut nodes,
        layout.shield_fill,
        hull.shields / hull.max_shields,
    );
    set_bar(&mut nodes, layout.armor_fill, hull.armor / hull.max_armor);
    set_bar(&mut nodes, layout.fuel_fill, fuel.current / fuel.max);

    let mut quest_text = String::new();
    if let Some(quests) = active_quests {
        for q in &quests.quests {
            let timer = q
                .remaining_secs
                .map(|s| format!(" [{}s]", s.ceil().max(0.0) as u32))
                .unwrap_or_default();
            let detail = match &q.target {
                QuestTarget::Delivery {
                    commodity,
                    qty,
                    destination_system,
                    destination_planet,
                } => format!(
                    "contract: {} {} -> {} ({}){}",
                    qty, commodity, destination_planet, destination_system, timer
                ),
                QuestTarget::Bounty {
                    system,
                    target,
                    killed,
                } => {
                    if *killed {
                        format!(
                            "contract: return to {} (+{} cr)",
                            q.giver_planet, q.reward_credits
                        )
                    } else {
                        format!("contract: hunt {} in {}{}", target, system, timer)
                    }
                }
            };
            quest_text.push_str(&detail);
            quest_text.push('\n');
        }
    }
    set_text(
        &mut texts,
        layout.quest_line,
        quest_text.trim_end().to_owned(),
    );

    let mut prompts = Vec::new();
    if let Some(docked_at) = docked {
        prompts.push("U — undock".to_owned());
        if encounter.is_some() {
            prompts.push(
                if docked_at.kind == DockableKind::Station {
                    "T — master"
                } else {
                    "T — chief"
                }
                .to_owned(),
            );
        }
    } else {
        let candidate = planets
            .iter()
            .filter(|(tag, planet_position)| {
                position.0.distance(planet_position.0) <= LAND_RANGE + tag.radius
            })
            .min_by_key(|(_, planet_position)| position.0.distance(planet_position.0).to_bits());
        if let Some((planet, _)) = candidate {
            prompts.push(format!(
                "L — {} {}",
                if planet.kind == DockableKind::Station {
                    "dock at"
                } else {
                    "land on"
                },
                planet.name
            ));
        }
        prompts.push("J — warp drive".to_owned());
    }
    set_text(&mut texts, layout.prompt_line, prompts.join("   "));
}

fn set_notice(texts: &mut Query<&mut Text>, layout: &HudLayout, board: &Noticeboard) {
    let fresh = board
        .at
        .is_some_and(|at| at.elapsed() < Duration::from_secs(5));
    let text = if fresh && !board.text.is_empty() {
        board.text.clone()
    } else {
        String::new()
    };
    set_text(texts, layout.notice_line, text);
}
