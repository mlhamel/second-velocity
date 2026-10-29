use bevy::prelude::*;

#[derive(Resource)]
pub struct UiFont(pub Handle<Font>);

pub struct UiFontPlugin;

impl Plugin for UiFontPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_ui_font);
    }
}

fn load_ui_font(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(UiFont(assets.load("fonts/press-start-2p.ttf")));
}

pub fn loaded(fonts: &Assets<Font>, font: &UiFont) -> bool {
    fonts.contains(&font.0)
}

pub fn text_node(
    parent: &mut ChildSpawnerCommands,
    font: &UiFont,
    value: impl Into<String>,
    size: f32,
    color: Color,
) -> Entity {
    parent
        .spawn((
            Text(value.into()),
            TextFont {
                font: bevy::text::FontSource::Handle(font.0.clone()),
                font_size: bevy::text::FontSize::Px(size),
                ..default()
            },
            TextColor(color),
        ))
        .id()
}
