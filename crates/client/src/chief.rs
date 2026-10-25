use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::prelude::*;
use sv_protocol::{ChiefEncounter, ChiefMood, ShipInfo};

use crate::MyNetworkId;
use crate::textures::Textures;

const PANEL_WIDTH: f32 = 340.0;
const PORTRAIT_BASE_SIZE: f32 = 84.0;
const CYAN: Color = Color::srgb(0.55, 1.0, 0.85);
const AMBER: Color = Color::srgb(1.0, 0.85, 0.5);
const BODY: Color = Color::srgb(0.92, 0.93, 0.96);
const DIM: Color = Color::srgb(0.5, 0.55, 0.65);

fn load_portrait(assets: &AssetServer, path: &str) -> Handle<Image> {
    assets
        .load_builder()
        .with_settings(|settings: &mut ImageLoaderSettings| {
            settings.sampler = ImageSampler::linear();
        })
        .load(path.to_owned())
}

#[derive(Resource, Default)]
pub struct ChiefPanelState {
    pub visible: bool,
    last_signature: Option<String>,
}

#[derive(Resource)]
struct ChiefPanelLayout {
    root: Entity,
    portrait: Entity,
    title_text: Entity,
    mood_text: Entity,
    dialog_text: Entity,
    options_text: Entity,
    hint_text: Entity,
}

pub struct ChiefPlugin;

impl Plugin for ChiefPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChiefPanelState>()
            .add_systems(Startup, spawn_chief_panel)
            .add_systems(
                Update,
                (sync_chief_panel, animate_chief_portrait)
                    .chain()
                    .before(crate::input::poll_action_input)
                    .before(crate::shops::ShopSystems),
            );
    }
}

fn spawn_chief_panel(mut commands: Commands) {
    let mut portrait_id = None;
    let mut title_id = None;
    let mut mood_id = None;
    let mut dialog_id = None;
    let mut options_id = None;
    let mut hint_id = None;

    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                top: Val::Px(252.0),
                width: Val::Px(PANEL_WIDTH),
                padding: UiRect::all(Val::Px(14.0)),
                border: UiRect::all(Val::Px(1.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.05, 0.1, 0.9)),
            BorderColor::all(Color::srgba(0.35, 0.75, 0.85, 0.6)),
        ))
        .with_children(|panel| {
            panel
                .spawn(Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(12.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|header| {
                    portrait_id = Some(
                        header
                            .spawn((
                                ImageNode::default(),
                                Node {
                                    width: Val::Px(PORTRAIT_BASE_SIZE),
                                    height: Val::Px(PORTRAIT_BASE_SIZE),
                                    top: Val::Px(0.0),
                                    ..default()
                                },
                            ))
                            .id(),
                    );
                    header
                        .spawn(Node {
                            display: Display::Flex,
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            ..default()
                        })
                        .with_children(|meta| {
                            title_id = Some(
                                meta.spawn((
                                    Text(String::new()),
                                    TextFont {
                                        font_size: bevy::text::FontSize::Px(18.0),
                                        ..default()
                                    },
                                    TextColor(CYAN),
                                ))
                                .id(),
                            );
                            mood_id = Some(
                                meta.spawn((
                                    Text(String::new()),
                                    TextFont {
                                        font_size: bevy::text::FontSize::Px(13.0),
                                        ..default()
                                    },
                                    TextColor(AMBER),
                                ))
                                .id(),
                            );
                        });
                });

            dialog_id = Some(
                panel
                    .spawn((
                        Text(String::new()),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(BODY),
                    ))
                    .id(),
            );

            options_id = Some(
                panel
                    .spawn((
                        Text(String::new()),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(14.0),
                            ..default()
                        },
                        TextColor(CYAN),
                    ))
                    .id(),
            );

            hint_id = Some(
                panel
                    .spawn((
                        Text("Esc — dismiss (market keys)   T — reopen".to_owned()),
                        TextFont {
                            font_size: bevy::text::FontSize::Px(11.0),
                            ..default()
                        },
                        TextColor(DIM),
                    ))
                    .id(),
            );
        })
        .id();

    commands.insert_resource(ChiefPanelLayout {
        root,
        portrait: portrait_id.expect("portrait node"),
        title_text: title_id.expect("title node"),
        mood_text: mood_id.expect("mood node"),
        dialog_text: dialog_id.expect("dialog node"),
        options_text: options_id.expect("options node"),
        hint_text: hint_id.expect("hint node"),
    });
}

fn mood_label(mood: ChiefMood) -> &'static str {
    match mood {
        ChiefMood::Offer => "SURPLUS OFFER",
        ChiefMood::Tribute => "PORT LEVY",
        ChiefMood::Quest => "CONTRACT BRIEFING",
        ChiefMood::TurnIn => "CONTRACT COMPLETION",
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_chief_panel(
    my_id: Res<MyNetworkId>,
    keys: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
    textures: Option<Res<Textures>>,
    layout: Res<ChiefPanelLayout>,
    mut state: ResMut<ChiefPanelState>,
    ships: Query<(&ShipInfo, Option<&ChiefEncounter>)>,
    mut nodes: Query<&mut Node>,
    mut images: Query<&mut ImageNode>,
    mut texts: Query<&mut Text>,
) {
    let encounter = ships
        .iter()
        .find(|(info, _)| info.owner == Some(my_id.0))
        .and_then(|(_, enc)| enc);

    let Some(encounter) = encounter else {
        state.visible = false;
        state.last_signature = None;
        if let Ok(mut root_node) = nodes.get_mut(layout.root) {
            root_node.display = Display::None;
        }
        return;
    };

    let signature = format!(
        "{}:{:?}:{}",
        encounter.chief_name, encounter.mood, encounter.text
    );
    if state.last_signature.as_ref() != Some(&signature) {
        state.visible = true;
        state.last_signature = Some(signature);

        if let Ok(mut image_node) = images.get_mut(layout.portrait) {
            image_node.image = match (&encounter.portrait, textures.as_ref()) {
                (Some(path), _) => load_portrait(&asset_server, path),
                (None, Some(tex)) => tex.chief.clone(),
                (None, None) => Handle::default(),
            };
        }
        if let Ok(mut text) = texts.get_mut(layout.title_text) {
            text.0 = encounter.chief_name.clone();
        }
        if let Ok(mut text) = texts.get_mut(layout.mood_text) {
            text.0 = mood_label(encounter.mood).to_owned();
        }
        if let Ok(mut text) = texts.get_mut(layout.dialog_text) {
            text.0 = encounter.text.clone();
        }
        if let Ok(mut text) = texts.get_mut(layout.options_text) {
            let mut formatted = String::new();
            for (idx, opt) in encounter.options.iter().enumerate() {
                formatted.push_str(&format!("[{}] {}\n", idx + 1, opt.label));
            }
            text.0 = formatted.trim_end().to_owned();
        }
        let _ = layout.hint_text;
    }

    if keys.just_pressed(KeyCode::Escape) {
        state.visible = false;
    } else if keys.just_pressed(KeyCode::KeyT) {
        state.visible = true;
    }

    if let Ok(mut root_node) = nodes.get_mut(layout.root) {
        root_node.display = if state.visible {
            Display::Flex
        } else {
            Display::None
        };
    }
}

fn animate_chief_portrait(
    time: Res<Time>,
    state: Res<ChiefPanelState>,
    layout: Res<ChiefPanelLayout>,
    mut nodes: Query<&mut Node>,
) {
    if !state.visible {
        return;
    }
    let Ok(mut portrait) = nodes.get_mut(layout.portrait) else {
        return;
    };
    let t = time.elapsed_secs();
    let bob = (t * 2.4).sin() * 3.0;
    let pulse = 1.0 + (t * 1.8).sin() * 0.025;
    portrait.top = Val::Px(bob);
    portrait.width = Val::Px(PORTRAIT_BASE_SIZE * pulse);
    portrait.height = Val::Px(PORTRAIT_BASE_SIZE * pulse);
}
