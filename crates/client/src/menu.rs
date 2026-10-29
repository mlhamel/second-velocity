use bevy::prelude::*;

use crate::chief::ChiefPanelState;
use crate::textures::Textures;
use crate::ui::{self, UiFont};

const CYAN: Color = Color::srgb(0.55, 1.0, 0.85);
const AMBER: Color = Color::srgb(1.0, 0.85, 0.5);
const VALUE: Color = Color::srgb(0.95, 0.95, 0.95);
const DIM: Color = Color::srgb(0.42, 0.44, 0.5);

const TITLE_SIZE: f32 = 40.0;
const HEADING_SIZE: f32 = 24.0;
const ITEM_SIZE: f32 = 16.0;
const FOOTER_SIZE: f32 = 8.0;

const TITLE_GLOW: Color = Color::srgb(1.5, 1.15, 0.55);
const ITEMS_GLOW: Color = Color::srgb(0.5, 1.35, 1.0);

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Menu,
    InGame,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuPhase {
    #[default]
    Title,
    Pause,
}

#[derive(Resource, Default)]
pub struct MenuSelection(pub(crate) usize);

#[derive(Component)]
struct MenuVisual;

#[derive(Resource)]
pub(crate) struct MenuLayout {
    items: Vec<Entity>,
    cursors: Vec<Entity>,
    glow_title: Entity,
    glow_items: Entity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuItem {
    Launch,
    Resume,
    Quit,
}

fn menu_items(phase: MenuPhase) -> [MenuItem; 2] {
    match phase {
        MenuPhase::Title => [MenuItem::Launch, MenuItem::Quit],
        MenuPhase::Pause => [MenuItem::Resume, MenuItem::Quit],
    }
}

fn item_label(item: MenuItem) -> &'static str {
    match item {
        MenuItem::Launch => "LAUNCH PILOT",
        MenuItem::Resume => "RESUME",
        MenuItem::Quit => "QUIT",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    EnterGame,
    Quit,
}

fn apply_menu_input(
    phase: MenuPhase,
    selection: &mut usize,
    keys: &ButtonInput<KeyCode>,
) -> Option<MenuAction> {
    let len = menu_items(phase).len();
    if keys.any_just_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        *selection = (*selection + len - 1) % len;
    }
    if keys.any_just_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        *selection = (*selection + 1) % len;
    }
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]) {
        return match menu_items(phase)[*selection] {
            MenuItem::Launch | MenuItem::Resume => Some(MenuAction::EnterGame),
            MenuItem::Quit => Some(MenuAction::Quit),
        };
    }
    if keys.just_pressed(KeyCode::Escape) && phase == MenuPhase::Pause {
        return Some(MenuAction::EnterGame);
    }
    None
}

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_resource::<MenuPhase>()
            .init_resource::<MenuSelection>()
            .add_systems(
                Update,
                (spawn_menu, menu_input, tick_menu, menu_camera_drift)
                    .chain()
                    .run_if(in_state(AppState::Menu)),
            )
            .add_systems(OnExit(AppState::Menu), despawn_menu)
            .add_systems(
                Update,
                open_pause_menu
                    .run_if(in_state(AppState::InGame))
                    .before(crate::chief::sync_chief_panel),
            );
    }
}

fn glow_sprite(textures: &Textures, color: Color, size: f32, y: f32) -> impl Bundle {
    let mut sprite = Sprite::from_image(textures.glow.clone());
    sprite.color = color;
    sprite.custom_size = Some(Vec2::splat(size));
    (
        MenuVisual,
        sprite,
        Transform::from_xyz(0.0, y, -5.0),
        Visibility::Visible,
    )
}

fn spawn_menu(
    mut commands: Commands,
    fonts: ResMut<Assets<Font>>,
    ui_font: Res<UiFont>,
    textures: Res<Textures>,
    phase: Res<MenuPhase>,
    existing: Query<(), With<MenuVisual>>,
) {
    if !existing.is_empty() || !ui::loaded(&fonts, &ui_font) {
        return;
    }

    let glow_title = commands
        .spawn(glow_sprite(&textures, TITLE_GLOW, 420.0, 110.0))
        .id();
    let glow_items = commands
        .spawn(glow_sprite(&textures, ITEMS_GLOW, 250.0, -80.0))
        .id();

    let mut items = Vec::new();
    let mut cursors = Vec::new();

    commands
        .spawn((
            MenuVisual,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.04, 0.55)),
        ))
        .with_children(|root| {
            match *phase {
                MenuPhase::Title => {
                    ui::text_node(root, &ui_font, "SECOND VELOCITY", TITLE_SIZE, CYAN);
                    ui::text_node(root, &ui_font, "THE LIVING GALAXY", ITEM_SIZE, AMBER);
                }
                MenuPhase::Pause => {
                    ui::text_node(root, &ui_font, "PAUSED", HEADING_SIZE, AMBER);
                }
            }

            for item in menu_items(*phase) {
                root.spawn(Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(10.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    cursors.push(ui::text_node(row, &ui_font, "> ", ITEM_SIZE, CYAN));
                    items.push(ui::text_node(
                        row,
                        &ui_font,
                        item_label(item),
                        ITEM_SIZE,
                        DIM,
                    ));
                });
            }

            let hint = match *phase {
                MenuPhase::Title => "W/S OR ARROWS SELECT   ENTER CONFIRM",
                MenuPhase::Pause => "ENTER CONFIRM   ESC RESUME",
            };
            let version = format!("SECOND VELOCITY v{}", env!("CARGO_PKG_VERSION"));
            root.spawn((Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(34.0),
                width: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },))
                .with_children(|footer| {
                    ui::text_node(footer, &ui_font, hint, FOOTER_SIZE, DIM);
                    ui::text_node(footer, &ui_font, version, FOOTER_SIZE, DIM);
                });
        });

    commands.insert_resource(MenuLayout {
        items,
        cursors,
        glow_title,
        glow_items,
    });
}

fn despawn_menu(mut commands: Commands, menu: Query<Entity, With<MenuVisual>>) {
    for entity in &menu {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<MenuLayout>();
}

fn menu_input(
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<MenuPhase>,
    mut selection: ResMut<MenuSelection>,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(action) = apply_menu_input(*phase, &mut selection.0, &keys) else {
        return;
    };
    match action {
        MenuAction::EnterGame => next_state.set(AppState::InGame),
        MenuAction::Quit => {
            exit.write(AppExit::Success);
        }
    }
}

fn tick_menu(
    time: Res<Time>,
    selection: Res<MenuSelection>,
    layout: Option<Res<MenuLayout>>,
    mut texts: Query<&mut Text>,
    mut text_colors: Query<&mut TextColor>,
    mut sprites: Query<&mut Sprite>,
) {
    let Some(layout) = layout else {
        return;
    };
    let t = time.elapsed_secs();

    let title_pulse = 0.9 + 0.1 * (t * 1.7).sin();
    if let Ok(mut sprite) = sprites.get_mut(layout.glow_title) {
        sprite.color = Color::srgb(1.5 * title_pulse, 1.15 * title_pulse, 0.55 * title_pulse);
    }
    let items_pulse = 0.85 + 0.15 * (t * 1.3).sin();
    if let Ok(mut sprite) = sprites.get_mut(layout.glow_items) {
        sprite.color = Color::srgb(0.5 * items_pulse, 1.35 * items_pulse, items_pulse);
    }

    let blink = (t * 2.2).sin() > -0.35;
    for (index, (&item, &cursor)) in layout.items.iter().zip(layout.cursors.iter()).enumerate() {
        let selected = index == selection.0;
        if let Ok(mut color) = text_colors.get_mut(item) {
            color.0 = if selected { CYAN } else { VALUE };
        }
        if let Ok(mut color) = text_colors.get_mut(cursor) {
            color.0 = if selected && blink {
                CYAN
            } else {
                Color::srgba(0.55, 1.0, 0.85, 0.0)
            };
        }
        if let Ok(mut text) = texts.get_mut(cursor) {
            text.0 = if selected && blink { "> " } else { "  " }.to_owned();
        }
    }
}

fn menu_camera_drift(time: Res<Time>, mut camera: Query<&mut Transform, With<Camera2d>>) {
    let t = time.elapsed_secs();
    for mut transform in &mut camera {
        transform.translation.x = (t * 0.13).sin() * 22.0;
        transform.translation.y = (t * 0.09).cos() * 14.0;
    }
}

fn open_pause_menu(
    keys: Res<ButtonInput<KeyCode>>,
    chief: Res<ChiefPanelState>,
    mut selection: ResMut<MenuSelection>,
    mut phase: ResMut<MenuPhase>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if !keys.just_pressed(KeyCode::Escape) || chief.visible {
        return;
    }
    selection.0 = 0;
    *phase = MenuPhase::Pause;
    next_state.set(AppState::Menu);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(pressed: &[KeyCode]) -> ButtonInput<KeyCode> {
        let mut keys = ButtonInput::default();
        for key in pressed {
            keys.press(*key);
        }
        keys
    }

    #[test]
    fn selection_wraps_in_both_directions() {
        let mut selection = 1;
        let up = keys(&[KeyCode::ArrowUp]);
        apply_menu_input(MenuPhase::Title, &mut selection, &up);
        assert_eq!(selection, 0);
        apply_menu_input(MenuPhase::Title, &mut selection, &up);
        assert_eq!(selection, 1);
        let down = keys(&[KeyCode::ArrowDown]);
        apply_menu_input(MenuPhase::Title, &mut selection, &down);
        assert_eq!(selection, 0);
    }

    #[test]
    fn confirm_enters_game_or_quits_from_either_item() {
        let enter = keys(&[KeyCode::Enter]);
        let mut launch = 0;
        assert_eq!(
            apply_menu_input(MenuPhase::Title, &mut launch, &enter),
            Some(MenuAction::EnterGame)
        );
        let mut quit = 1;
        assert_eq!(
            apply_menu_input(MenuPhase::Pause, &mut quit, &enter),
            Some(MenuAction::Quit)
        );
        let mut resume = 0;
        assert_eq!(
            apply_menu_input(MenuPhase::Pause, &mut resume, &enter),
            Some(MenuAction::EnterGame)
        );
    }

    #[test]
    fn escape_resumes_only_from_pause() {
        let escape = keys(&[KeyCode::Escape]);
        let mut title = 0;
        assert_eq!(
            apply_menu_input(MenuPhase::Title, &mut title, &escape),
            None
        );
        let mut pause = 1;
        assert_eq!(
            apply_menu_input(MenuPhase::Pause, &mut pause, &escape),
            Some(MenuAction::EnterGame)
        );
        assert_eq!(pause, 1, "escape resumes without moving the selection");
    }
}
