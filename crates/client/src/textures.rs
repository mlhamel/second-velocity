use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

#[derive(Resource)]
pub struct Textures {
    pub ship: Handle<Image>,
    pub planet: Handle<Image>,
    pub star: Handle<Image>,
    pub glow: Handle<Image>,
    pub chief: Handle<Image>,
    pub meteorites: [Handle<Image>; 3],
}

pub(crate) fn create_textures(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    content: Res<crate::input::ClientContent>,
) {
    let ship = images.add(ship_image(96));
    let planet = images.add(planet_image(128));
    let star = images.add(star_image(128));
    let glow = images.add(glow_image(64));
    let chief = images.add(chief_image(128));
    let meteorites = [
        images.add(meteorite_image(96, 0)),
        images.add(meteorite_image(96, 1)),
        images.add(meteorite_image(96, 2)),
    ];
    for system in &content.0.systems {
        for accent in crate::sky_gen::nebula_accents(&system.id, 3) {
            let mut nebula = Sprite::from_image(glow.clone());
            nebula.color = accent.color;
            nebula.custom_size = Some(Vec2::splat(accent.size));
            commands.spawn((
                nebula,
                Transform::from_xyz(accent.position.x, accent.position.y, crate::sky::SKY_Z),
            ));
        }
    }

    commands.insert_resource(Textures {
        ship,
        planet,
        star,
        glow,
        chief,
        meteorites,
    });
}

fn meteorite_image(size: u32, variant: usize) -> Image {
    let mut data = vec![0; (size * size * 4) as usize];
    let profiles: [&[(f32, f32)]; 3] = [
        &[
            (0.92, 0.48),
            (0.78, 0.20),
            (0.48, 0.10),
            (0.20, 0.22),
            (0.08, 0.46),
            (0.18, 0.76),
            (0.44, 0.90),
            (0.74, 0.84),
            (0.88, 0.66),
        ],
        &[
            (0.90, 0.42),
            (0.70, 0.14),
            (0.36, 0.12),
            (0.12, 0.34),
            (0.10, 0.64),
            (0.28, 0.88),
            (0.62, 0.90),
            (0.84, 0.72),
        ],
        &[
            (0.94, 0.52),
            (0.80, 0.24),
            (0.54, 0.12),
            (0.24, 0.18),
            (0.10, 0.42),
            (0.14, 0.72),
            (0.38, 0.88),
            (0.68, 0.86),
            (0.86, 0.70),
        ],
    ];
    let craters: [&[(f32, f32, f32)]; 3] = [
        &[(0.62, 0.38, 0.13), (0.34, 0.60, 0.10), (0.56, 0.68, 0.08)],
        &[(0.42, 0.38, 0.14), (0.64, 0.60, 0.11), (0.30, 0.64, 0.07)],
        &[(0.58, 0.44, 0.12), (0.34, 0.42, 0.09), (0.50, 0.70, 0.10)],
    ];
    let s = size as f32;
    let poly: Vec<(f32, f32)> = profiles[variant % 3]
        .iter()
        .map(|(x, y)| (x * s, y * s))
        .collect();

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            if !point_in_polygon((px, py), &poly) {
                continue;
            }
            let nx = (px / s) - 0.5;
            let ny = (py / s) - 0.5;
            let dist = (nx * nx + ny * ny).sqrt();
            // Directional shading from upper-right (+x, -y) plus faceted grain
            let light = (0.68 + nx * 0.45 - ny * 0.35 - dist * 0.35).clamp(0.28, 1.0);
            let grain = (((x.wrapping_mul(17) ^ y.wrapping_mul(31)) & 7) as f32 - 3.5) * 3.0;
            let mut r = (168.0 * light + grain).clamp(40.0, 220.0);
            let mut g = (152.0 * light + grain).clamp(36.0, 205.0);
            let mut b = (136.0 * light + grain).clamp(32.0, 190.0);

            for &(cx, cy, cr) in craters[variant % 3] {
                let cdx = (px / s) - cx;
                let cdy = (py / s) - cy;
                let cdist = (cdx * cdx + cdy * cdy).sqrt();
                if cdist < cr {
                    let rim = cdist / cr;
                    let factor = if rim > 0.78 { 1.12 } else { 0.58 + 0.25 * rim };
                    r *= factor;
                    g *= factor;
                    b *= factor;
                }
            }

            let index = ((y * size + x) * 4) as usize;
            data[index] = r as u8;
            data[index + 1] = g as u8;
            data[index + 2] = b as u8;
            data[index + 3] = 255;
        }
    }
    image_from_rgba(size, data)
}

fn chief_image(size: u32) -> Image {
    let mut data = vec![0; (size * size * 4) as usize];
    let shoulders = [
        (0.18, 0.92),
        (0.24, 0.64),
        (0.40, 0.56),
        (0.60, 0.56),
        (0.76, 0.64),
        (0.82, 0.92),
    ];
    fill_circle(&mut data, size, 0.5, 0.52, 0.44, 48, 255);
    fill_polygon(&mut data, size, &shoulders, 180);
    fill_circle(&mut data, size, 0.5, 0.38, 0.16, 225, 255);
    image_from_rgba(size, data)
}

fn ship_image(size: u32) -> Image {
    let mut data = vec![0; (size * size * 4) as usize];
    let hull = [
        (0.96, 0.5),
        (0.52, 0.36),
        (0.16, 0.4),
        (0.16, 0.6),
        (0.52, 0.64),
    ];
    let wing_top = [(0.56, 0.37), (0.24, 0.06), (0.12, 0.1), (0.32, 0.42)];
    let wing_bottom = [(0.56, 0.63), (0.24, 0.94), (0.12, 0.9), (0.32, 0.58)];
    let engine = [(0.04, 0.45), (0.16, 0.45), (0.16, 0.55), (0.04, 0.55)];

    fill_polygon(&mut data, size, &wing_top, 210);
    fill_polygon(&mut data, size, &wing_bottom, 210);
    fill_polygon(&mut data, size, &hull, 255);
    fill_polygon(&mut data, size, &engine, 255);
    fill_circle(&mut data, size, 0.62, 0.5, 0.05, 70, 255);

    image_from_rgba(size, data)
}

fn planet_image(size: u32) -> Image {
    let mut data = vec![0; (size * size * 4) as usize];
    let s = size as f32;
    let radius = s * 0.44;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - s / 2.0;
            let dy = y as f32 + 0.5 - s / 2.0;
            let distance = (dx * dx + dy * dy).sqrt();
            let index = ((y * size + x) * 4) as usize;
            if distance <= radius {
                let shade = 0.55 + 0.45 * (1.0 - (distance / radius).powi(2)).sqrt();
                let value = (shade * 255.0) as u8;
                data[index] = value;
                data[index + 1] = value;
                data[index + 2] = value;
                data[index + 3] = 255;
            } else if distance <= radius * 1.12 {
                let alpha = ((1.0 - (distance - radius) / (radius * 0.12)) * 0.4 * 255.0) as u8;
                data[index] = 255;
                data[index + 1] = 255;
                data[index + 2] = 255;
                data[index + 3] = alpha;
            }
        }
    }
    image_from_rgba(size, data)
}

fn star_image(size: u32) -> Image {
    let mut data = vec![0; (size * size * 4) as usize];
    let s = size as f32;
    let disc = s * 0.30;
    let corona = s * 0.48;
    let center = s / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let distance = (dx * dx + dy * dy).sqrt();
            let index = ((y * size + x) * 4) as usize;
            if distance <= disc {
                let rim = (distance / disc).powi(3);
                let value = (1.0 - rim * 0.25) * 255.0;
                data[index] = value as u8;
                data[index + 1] = (value * 0.955) as u8;
                data[index + 2] = (value * 0.84) as u8;
                data[index + 3] = 255;
            } else if distance <= corona {
                let falloff = (1.0 - (distance - disc) / (corona - disc)).powi(3);
                data[index] = 255;
                data[index + 1] = (110.0 + 90.0 * falloff) as u8;
                data[index + 2] = (40.0 + 60.0 * falloff) as u8;
                data[index + 3] = (falloff * 0.85 * 255.0) as u8;
            }
        }
    }
    image_from_rgba(size, data)
}

fn glow_image(size: u32) -> Image {
    let mut data = vec![0; (size * size * 4) as usize];
    let s = size as f32;
    let radius = s / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - radius;
            let dy = y as f32 + 0.5 - radius;
            let distance = (dx * dx + dy * dy).sqrt() / radius;
            if distance <= 1.0 {
                let falloff = (1.0 - distance).powi(2);
                let index = ((y * size + x) * 4) as usize;
                data[index] = 255;
                data[index + 1] = (160.0 + 95.0 * falloff) as u8;
                data[index + 2] = (60.0 + 60.0 * falloff) as u8;
                data[index + 3] = (falloff * 255.0) as u8;
            }
        }
    }
    image_from_rgba(size, data)
}

fn fill_polygon(data: &mut [u8], size: u32, vertices: &[(f32, f32)], value: u8) {
    let s = size as f32;
    let scaled: Vec<(f32, f32)> = vertices.iter().map(|(x, y)| (x * s, y * s)).collect();
    for y in 0..size {
        for x in 0..size {
            if point_in_polygon((x as f32 + 0.5, y as f32 + 0.5), &scaled) {
                let index = ((y * size + x) * 4) as usize;
                data[index] = value;
                data[index + 1] = value;
                data[index + 2] = value;
                data[index + 3] = 255;
            }
        }
    }
}

fn fill_circle(data: &mut [u8], size: u32, cx: f32, cy: f32, radius: f32, value: u8, alpha: u8) {
    let s = size as f32;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - cx * s;
            let dy = y as f32 + 0.5 - cy * s;
            if (dx * dx + dy * dy).sqrt() <= radius * s {
                let index = ((y * size + x) * 4) as usize;
                data[index] = value;
                data[index + 1] = value;
                data[index + 2] = value;
                data[index + 3] = alpha;
            }
        }
    }
}

fn point_in_polygon(point: (f32, f32), vertices: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut j = vertices.len() - 1;
    for i in 0..vertices.len() {
        let (xi, yi) = vertices[i];
        let (xj, yj) = vertices[j];
        if (yi > point.1) != (yj > point.1) && point.0 < (xj - xi) * (point.1 - yi) / (yj - yi) + xi
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub(crate) fn image_from_rgba(size: u32, data: Vec<u8>) -> Image {
    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}
