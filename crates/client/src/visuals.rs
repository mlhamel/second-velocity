use crate::MyNetworkId;
use crate::input::ClientContent;
use crate::textures::Textures;
use bevy::color::Mix;
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use sv_data::ShipSize;
use sv_protocol::{
    BlackHoleTag, DockableKind, DockableTag, Explosion, Heading, Hull, Meteorite, PlanetTag,
    Position, Projectile, ShipInfo, ShipKind, StarTag,
};

const FLASH_SECS: f32 = 0.18;
const SPRITE_CANVAS: f32 = 128.0;

fn ship_size_index(size: ShipSize) -> usize {
    match size {
        ShipSize::Light => 0,
        ShipSize::Medium => 1,
        ShipSize::Heavy => 2,
        ShipSize::Capital => 3,
    }
}

pub struct VisualsPlugin;

impl Plugin for VisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_explosion).add_systems(
            Update,
            (
                attach_ship_visuals,
                (attach_planet_visuals, attach_station_visuals).chain(),
                attach_star_visuals,
                attach_black_hole_visuals,
                attach_projectile_visuals,
                attach_meteorite_visuals,
                detect_hits,
                detect_meteorite_hits,
                tick_flashes,
                debris_flight,
                sync_ship_transforms,
                (sync_planet_transforms, sync_station_transforms).chain(),
                sync_star_transforms,
                sync_black_hole_transforms,
                sync_projectile_transforms,
                sync_meteorite_transforms,
                parallax,
                camera_follow,
            )
                .chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use sv_data::{ContentPack, ShipSize};

    #[test]
    fn hull_swap_refreshes_existing_sprite_and_resets_damage_baseline() {
        let mut content = ContentPack::load_dir(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content"),
        )
        .expect("fixture content");
        let mut old = content.ship("shuttle").unwrap().clone();
        old.sprite = None;
        old.size = ShipSize::Light;
        let mut replacement = old.clone();
        replacement.id = "visual-test-heavy".into();
        replacement.size = ShipSize::Heavy;
        content.ships.retain(|ship| ship.id != old.id);
        content.ships.extend([old, replacement]);

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .insert_resource(ClientContent(content))
            .insert_resource(Textures {
                ships: std::array::from_fn(|_| default()),
                planet: default(),
                station: default(),
                star: default(),
                black_hole: default(),
                glow: default(),
                chief: default(),
                meteorites: default(),
            })
            .add_systems(Update, (attach_ship_visuals, detect_hits).chain());
        let entity = app
            .world_mut()
            .spawn((
                Remote,
                ShipInfo {
                    def_id: "shuttle".into(),
                    kind: ShipKind::Player,
                    owner: Some(1),
                    name: "Test ship".into(),
                },
                Hull {
                    shields: 40.0,
                    max_shields: 40.0,
                    armor: 25.0,
                    max_armor: 25.0,
                },
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Sprite>(entity).unwrap().custom_size,
            Some(Vec2::splat(ShipSize::Light.world_size())),
        );
        app.world_mut().get_mut::<ShipInfo>(entity).unwrap().def_id = "visual-test-heavy".into();
        app.world_mut().get_mut::<Hull>(entity).unwrap().shields = 20.0;
        app.update();
        assert_eq!(
            app.world().get::<Sprite>(entity).unwrap().custom_size,
            Some(Vec2::splat(ShipSize::Heavy.world_size())),
        );
        let flash = app.world().get::<FlashState>(entity).unwrap();
        assert_eq!(flash.remaining, 0.0, "a hull swap is not a combat hit");
        assert_eq!(flash.last_shields, 20.0);
        app.world_mut().get_mut::<Hull>(entity).unwrap().shields = 19.0;
        app.update();
        assert_eq!(
            app.world().get::<FlashState>(entity).unwrap().remaining,
            FLASH_SECS
        );
    }
}

#[derive(Component)]
struct ShipVisual;

#[derive(Component)]
struct PlanetVisual;

#[derive(Component)]
struct StationVisual;

#[derive(Component)]
struct StarVisual;

#[derive(Component)]
struct BlackHoleVisual;

#[derive(Component)]
struct ProjectileVisual;

#[derive(Component)]
struct MeteoriteVisual;

#[derive(Component)]
struct FlashState {
    base_color: Color,
    last_shields: f32,
    last_armor: f32,
    remaining: f32,
}

#[derive(Component)]
struct Debris {
    velocity: Vec2,
    ttl: f32,
}

#[derive(Component)]
struct StarLayer(f32);

fn kind_color(kind: ShipKind) -> Color {
    match kind {
        ShipKind::Player => Color::srgb(0.55, 1.0, 0.85),
        ShipKind::Pirate => Color::srgb(1.0, 0.4, 0.3),
        ShipKind::Police => Color::srgb(0.45, 0.65, 1.0),
        ShipKind::Merchant => Color::srgb(1.0, 0.82, 0.38),
        ShipKind::Defender => Color::srgb(0.4, 0.85, 1.0),
        ShipKind::Explorer => Color::srgb(0.75, 0.55, 1.0),
    }
}

type ChangedShips<'w, 's> =
    Query<'w, 's, (Entity, &'static ShipInfo, &'static Hull), (Changed<ShipInfo>, With<Remote>)>;

fn attach_ship_visuals(
    mut commands: Commands,
    textures: Res<Textures>,
    asset_server: Res<AssetServer>,
    content: Res<ClientContent>,
    changed: ChangedShips,
) {
    for (entity, info, hull) in &changed {
        let ship_def = content.0.ship(&info.def_id);
        let (world_px, sprite_px, fallback) = match ship_def {
            Some(def) => (
                def.size.world_size(),
                SPRITE_CANVAS,
                textures.ships[ship_size_index(def.size)].clone(),
            ),
            None => (48.0, 48.0, textures.ships[1].clone()),
        };
        let (image, base_color, custom_px) = match ship_def.and_then(|def| def.sprite.clone()) {
            Some(path) => (asset_server.load(path), Color::WHITE, sprite_px),
            None => (fallback, kind_color(info.kind), world_px),
        };
        let mut sprite = Sprite::from_image(image);
        sprite.color = base_color;
        sprite.custom_size = Some(Vec2::splat(custom_px));
        commands.entity(entity).insert((
            ShipVisual,
            sprite,
            FlashState {
                base_color,
                last_shields: hull.shields,
                last_armor: hull.armor,
                remaining: 0.0,
            },
        ));
    }
}

type AddedPlanets<'w, 's> =
    Query<'w, 's, (Entity, &'static PlanetTag), (Added<PlanetTag>, With<Remote>)>;

fn attach_planet_visuals(
    mut commands: Commands,
    textures: Res<Textures>,
    asset_server: Res<AssetServer>,
    content: Res<ClientContent>,
    added: AddedPlanets,
) {
    for (entity, planet) in &added {
        let sprite_path = content
            .0
            .system(&planet.system)
            .and_then(|system| system.planets.iter().find(|p| p.id == planet.id))
            .and_then(|p| p.sprite.clone());
        let (image, color) = match sprite_path {
            Some(path) => (asset_server.load(path), Color::WHITE),
            None => {
                let palette = [
                    Color::srgb(0.2, 0.45, 0.9),
                    Color::srgb(0.55, 0.4, 0.3),
                    Color::srgb(0.35, 0.55, 0.35),
                    Color::srgb(0.6, 0.55, 0.5),
                ];
                let seed = planet.id.bytes().map(|b| b as usize).sum::<usize>();
                (textures.planet.clone(), palette[seed % palette.len()])
            }
        };
        let mut sprite = Sprite::from_image(image);
        sprite.color = color;
        sprite.custom_size = Some(Vec2::splat(planet.radius * 2.0));
        commands.entity(entity).insert((PlanetVisual, sprite));
    }
}

type AddedPorts<'w, 's> =
    Query<'w, 's, (Entity, &'static DockableTag), (Added<DockableTag>, With<Remote>)>;

fn attach_station_visuals(
    mut commands: Commands,
    textures: Res<Textures>,
    asset_server: Res<AssetServer>,
    content: Res<ClientContent>,
    added: AddedPorts,
) {
    for (entity, port) in &added {
        if port.kind != DockableKind::Station {
            continue;
        }
        let path = content
            .0
            .system(&port.system)
            .and_then(|s| s.stations.iter().find(|s| s.id == port.id))
            .and_then(|s| s.sprite.clone());
        let image = path.map_or_else(|| textures.station.clone(), |path| asset_server.load(path));
        let mut sprite = Sprite::from_image(image);
        sprite.custom_size = Some(Vec2::splat(SPRITE_CANVAS));
        commands.entity(entity).insert((StationVisual, sprite));
    }
}

fn sync_station_transforms(mut stations: Query<(&Position, &mut Transform), With<StationVisual>>) {
    for (position, mut transform) in &mut stations {
        transform.translation = position.0.extend(-0.5);
    }
}

type AddedStars<'w, 's> = Query<'w, 's, (Entity, &'static StarTag), (Added<StarTag>, With<Remote>)>;

fn attach_star_visuals(
    mut commands: Commands,
    textures: Res<Textures>,
    asset_server: Res<AssetServer>,
    content: Res<ClientContent>,
    added: AddedStars,
) {
    for (entity, star) in &added {
        let sprite_path = content
            .0
            .system(&star.system)
            .and_then(|system| system.star.as_ref())
            .and_then(|s| s.sprite.clone());
        let image = match sprite_path {
            Some(path) => asset_server.load(path),
            None => textures.star.clone(),
        };
        let mut sprite = Sprite::from_image(image);
        // The texture's corona falloff reaches 48% of the sprite, so scale it
        // to make the visible halo match the 1.8x-radius heat-zone boundary.
        sprite.color = Color::srgb(1.6, 1.5, 1.35);
        sprite.custom_size = Some(Vec2::splat(star.radius * 3.75));
        commands.entity(entity).insert((StarVisual, sprite));
    }
}

type AddedBlackHoles<'w, 's> =
    Query<'w, 's, (Entity, &'static BlackHoleTag), (Added<BlackHoleTag>, With<Remote>)>;

fn attach_black_hole_visuals(
    mut commands: Commands,
    textures: Res<Textures>,
    asset_server: Res<AssetServer>,
    content: Res<ClientContent>,
    added: AddedBlackHoles,
) {
    for (entity, hole) in &added {
        let sprite_path = content
            .0
            .system(&hole.system)
            .and_then(|system| system.black_hole.as_ref())
            .and_then(|hole| hole.sprite.clone());
        let image = match sprite_path {
            Some(path) => asset_server.load(path),
            None => textures.black_hole.clone(),
        };
        let mut sprite = Sprite::from_image(image);
        // The opaque dark disc occupies 25% of the texture width in radius;
        // its visible edge is the authoritative event horizon.
        sprite.custom_size = Some(Vec2::splat(hole.radius * 4.0));
        commands.entity(entity).insert((BlackHoleVisual, sprite));
    }
}

fn attach_projectile_visuals(
    mut commands: Commands,
    textures: Res<Textures>,
    added: Query<Entity, (Added<Projectile>, With<Remote>)>,
) {
    for entity in &added {
        let mut sprite = Sprite::from_image(textures.glow.clone());
        sprite.color = Color::srgb(1.9, 1.35, 0.6);
        sprite.custom_size = Some(Vec2::splat(16.0));
        commands.entity(entity).insert((ProjectileVisual, sprite));
    }
}

type AddedMeteorites<'w, 's> =
    Query<'w, 's, (Entity, &'static Meteorite), (Added<Meteorite>, With<Remote>)>;

fn attach_meteorite_visuals(
    mut commands: Commands,
    textures: Res<Textures>,
    added: AddedMeteorites,
) {
    for (entity, meteorite) in &added {
        let variant_idx = (meteorite.variant as usize) % textures.meteorites.len();
        let mut sprite = Sprite::from_image(textures.meteorites[variant_idx].clone());
        sprite.color = Color::WHITE;
        sprite.custom_size = Some(Vec2::splat(meteorite.size.radius() * 2.2));
        commands.entity(entity).insert((
            MeteoriteVisual,
            sprite,
            FlashState {
                base_color: Color::WHITE,
                last_shields: meteorite.hp,
                last_armor: meteorite.hp,
                remaining: 0.0,
            },
        ));
    }
}

fn detect_hits(mut ships: Query<(&Hull, &mut FlashState), With<ShipVisual>>) {
    for (hull, mut flash) in &mut ships {
        if hull.shields < flash.last_shields || hull.armor < flash.last_armor {
            flash.remaining = FLASH_SECS;
        }
        flash.last_shields = hull.shields;
        flash.last_armor = hull.armor;
    }
}

fn detect_meteorite_hits(
    mut meteorites: Query<(&Meteorite, &mut FlashState), With<MeteoriteVisual>>,
) {
    for (meteorite, mut flash) in &mut meteorites {
        if meteorite.hp < flash.last_armor {
            flash.remaining = FLASH_SECS;
        }
        flash.last_shields = meteorite.hp;
        flash.last_armor = meteorite.hp;
    }
}

fn tick_flashes(time: Res<Time>, mut flashing: Query<(&mut FlashState, &mut Sprite)>) {
    let dt = time.delta_secs();
    for (mut flash, mut sprite) in &mut flashing {
        if flash.remaining > 0.0 {
            flash.remaining = (flash.remaining - dt).max(0.0);
            let factor = flash.remaining / FLASH_SECS;
            sprite.color = flash.base_color.mix(&Color::srgb(2.0, 0.45, 0.35), factor);
        } else {
            sprite.color = flash.base_color;
        }
    }
}

fn on_explosion(trigger: On<Explosion>, mut commands: Commands, textures: Res<Textures>) {
    let origin = Vec2::from(trigger.event().position);
    for index in 0..12u32 {
        let angle = index as f32 * std::f32::consts::FRAC_PI_6 + (index % 3) as f32 * 0.17;
        let speed = 60.0 + (index % 5) as f32 * 28.0;
        let mut sprite = Sprite::from_image(textures.glow.clone());
        sprite.custom_size = Some(Vec2::splat(8.0));
        sprite.color = Color::srgba(1.9, 1.1, 0.45, 0.95);
        commands.spawn((
            sprite,
            Transform::from_translation(origin.extend(5.0)),
            Debris {
                velocity: Vec2::new(angle.cos(), angle.sin()) * speed,
                ttl: 0.8,
            },
        ));
    }
}

fn debris_flight(
    mut commands: Commands,
    time: Res<Time>,
    mut debris: Query<(Entity, &mut Debris, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    for (entity, mut debris, mut transform, mut sprite) in &mut debris {
        debris.ttl -= dt;
        transform.translation += (debris.velocity * dt).extend(0.0);
        let alpha = (debris.ttl / 0.8).clamp(0.0, 1.0);
        sprite.color = Color::srgba(1.9, 1.1, 0.45, alpha);
        if debris.ttl <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn sync_ship_transforms(mut ships: Query<(&Position, &Heading, &mut Transform), With<ShipVisual>>) {
    for (position, heading, mut transform) in &mut ships {
        transform.translation = position.0.extend(0.0);
        transform.rotation = Quat::from_rotation_z(heading.0);
    }
}

fn sync_planet_transforms(mut planets: Query<(&Position, &mut Transform), With<PlanetVisual>>) {
    for (position, mut transform) in &mut planets {
        transform.translation = position.0.extend(0.0);
    }
}

fn sync_star_transforms(mut stars: Query<(&Position, &mut Transform), With<StarVisual>>) {
    for (position, mut transform) in &mut stars {
        transform.translation = position.0.extend(-2.0);
    }
}

fn sync_black_hole_transforms(
    mut holes: Query<(&Position, &mut Transform), With<BlackHoleVisual>>,
) {
    for (position, mut transform) in &mut holes {
        transform.translation = position.0.extend(-1.5);
    }
}

fn sync_projectile_transforms(
    mut projectiles: Query<(&Position, &Heading, &mut Transform), With<ProjectileVisual>>,
) {
    for (position, heading, mut transform) in &mut projectiles {
        transform.translation = position.0.extend(0.0);
        transform.rotation = Quat::from_rotation_z(heading.0);
    }
}

fn sync_meteorite_transforms(
    mut meteorites: Query<(&Position, &Heading, &mut Transform), With<MeteoriteVisual>>,
) {
    for (position, heading, mut transform) in &mut meteorites {
        transform.translation = position.0.extend(-1.0);
        transform.rotation = Quat::from_rotation_z(heading.0);
    }
}

fn parallax(
    camera: Query<&Transform, (With<Camera2d>, Without<StarLayer>)>,
    mut layers: Query<(&StarLayer, &mut Transform)>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    for (layer, mut transform) in &mut layers {
        transform.translation.x = camera.translation.x * (1.0 - layer.0);
        transform.translation.y = camera.translation.y * (1.0 - layer.0);
    }
}

fn camera_follow(
    my_id: Res<MyNetworkId>,
    my_ships: Query<(&ShipInfo, &Position)>,
    mut camera: Query<&mut Transform, (With<Camera2d>, Without<Position>)>,
) {
    let Some((_, position)) = my_ships
        .iter()
        .find(|(info, _)| info.owner == Some(my_id.0))
    else {
        return;
    };
    for mut transform in &mut camera {
        transform.translation = position.0.extend(0.0);
    }
}

pub fn spawn_starfield(mut commands: Commands) {
    for (factor, count, size, base_brightness) in [(0.25, 90, 2.0, 0.3), (0.55, 45, 3.0, 0.55)] {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        commands
            .spawn((
                StarLayer(factor),
                Transform::default(),
                GlobalTransform::default(),
                Visibility::default(),
            ))
            .with_children(|parent| {
                for _ in 0..count {
                    seed = seed
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    let x = (((seed >> 33) as i64 % 8000) as f32) - 4000.0;
                    let y = (((seed >> 13) as i64 % 6000) as f32) - 3000.0;
                    let brightness =
                        base_brightness + (((seed >> 29) & 0xFF) as f32 / u8::MAX as f32) * 0.35;
                    let warm = (seed >> 41) & 1 == 1;
                    let color = if warm {
                        Color::srgb(brightness, brightness * 0.92, brightness * 0.75)
                    } else {
                        Color::srgb(brightness * 0.8, brightness * 0.9, brightness)
                    };
                    parent.spawn((
                        Sprite::from_color(color, Vec2::splat(size)),
                        Transform::from_xyz(x, y, -20.0),
                    ));
                }
            });
    }
}
