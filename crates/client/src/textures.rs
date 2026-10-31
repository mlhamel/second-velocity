use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

#[derive(Resource)]
pub struct Textures {
    pub ships: [Handle<Image>; 4],
    pub planet: Handle<Image>,
    pub moon: Handle<Image>,
    pub ring_back: Handle<Image>,
    pub ring_front: Handle<Image>,
    pub station: Handle<Image>,
    pub star: Handle<Image>,
    pub black_hole: Handle<Image>,
    pub glow: Handle<Image>,
    pub chief: Handle<Image>,
    pub meteorites: [Handle<Image>; 3],
}

pub(crate) fn create_textures(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    content: Res<crate::input::ClientContent>,
) {
    let ships = [
        images.add(ship_image(36)),
        images.add(ship_image(48)),
        images.add(ship_image(68)),
        images.add(ship_image(92)),
    ];
    let planet = images.add(planet_image(180));
    let moon = images.add(moon_image(64));
    let ring_back = images.add(planet_ring_half_image(192, false));
    let ring_front = images.add(planet_ring_half_image(192, true));
    let station = images.add(station_image(128));
    let star = images.add(star_image(128));
    let black_hole = images.add(black_hole_image(128));
    let glow = images.add(glow_image(16));
    let nebula = images.add(nebula_image(64));
    let chief = images.add(chief_image(128));
    let meteorites = [
        images.add(meteorite_image(62, 0)),
        images.add(meteorite_image(38, 1)),
        images.add(meteorite_image(22, 2)),
    ];
    for system in &content.0.systems {
        for accent in crate::sky_gen::nebula_accents(&system.id, 3) {
            let mut nebula_sprite = Sprite::from_image(nebula.clone());
            nebula_sprite.color = accent.color;
            nebula_sprite.custom_size = Some(Vec2::splat(accent.size));
            commands.spawn((
                nebula_sprite,
                Transform::from_xyz(accent.position.x, accent.position.y, crate::sky::SKY_Z),
            ));
        }
    }

    commands.insert_resource(Textures {
        ships,
        planet,
        moon,
        ring_back,
        ring_front,
        station,
        star,
        black_hole,
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
    quantize_shading(&mut data, size, 3);
    outline_silhouette(&mut data, size, [30, 26, 24]);
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
    let mut image = image_from_rgba(size, data);
    image.sampler = ImageSampler::linear();
    image
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
    quantize_shading(&mut data, size, 4);
    outline_silhouette(&mut data, size, [24, 26, 34]);

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
    quantize_shading(&mut data, size, 5);
    image_from_rgba(size, data)
}

fn moon_image(size: u32) -> Image {
    let mut data = vec![0; (size * size * 4) as usize];
    let s = size as f32;
    let radius = s * 0.44;
    let craters = [
        (-0.16, -0.10, 0.11),
        (0.14, 0.08, 0.09),
        (-0.04, 0.20, 0.07),
    ];
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - s / 2.0;
            let dy = y as f32 + 0.5 - s / 2.0;
            let distance = (dx * dx + dy * dy).sqrt();
            let index = ((y * size + x) * 4) as usize;
            if distance <= radius {
                let nx = dx / radius;
                let ny = dy / radius;
                let light = ((1.0 - (distance / radius).powi(2)).sqrt() * 0.45
                    + (-nx * 0.35 - ny * 0.25)
                    + 0.55)
                    .clamp(0.28, 1.0);
                let mut shade = light;
                for (cx, cy, cr) in craters {
                    let cd = (dx / s - cx).hypot(dy / s - cy);
                    if cd <= cr {
                        shade *= 0.78;
                    }
                }
                let value = (shade * 255.0) as u8;
                data[index] = value;
                data[index + 1] = value;
                data[index + 2] = value;
                data[index + 3] = 255;
            }
        }
    }
    quantize_shading(&mut data, size, 4);
    outline_silhouette(&mut data, size, [26, 28, 34]);
    image_from_rgba(size, data)
}

fn planet_ring_half_image(size: u32, front_half: bool) -> Image {
    let mut data = vec![0; (size * size * 4) as usize];
    let s = size as f32;
    let center = s / 2.0;
    let max_r = s * 0.47;
    let min_r = s * 0.28;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            // In image coordinates, y < center is the top half (rendered behind the planet at z < 0),
            // and y >= center is the bottom half (rendered in front of the planet at z > 0).
            let in_half = if front_half { dy >= 0.0 } else { dy <= 0.0 };
            if !in_half {
                continue;
            }
            let r = dx.hypot(dy);
            if r < min_r || r > max_r {
                continue;
            }
            let t = (r - min_r) / (max_r - min_r);
            // Cassini-style division gap around t in 0.56..0.64.
            if (0.56..=0.64).contains(&t) {
                continue;
            }
            let band = ((t * 14.0).sin() * 0.15 + 0.85).clamp(0.55, 1.0);
            let checker = ((x + y) % 2 == 0) as u8;
            let alpha = if (0.22..=0.52).contains(&t) {
                215
            } else if checker == 0 {
                175
            } else {
                125
            };
            let v = (band * 255.0) as u8;
            let index = ((y * size + x) * 4) as usize;
            data[index] = v;
            data[index + 1] = v;
            data[index + 2] = v;
            data[index + 3] = alpha;
        }
    }
    image_from_rgba(size, data)
}

fn station_image(size: u32) -> Image {
    let mut data = vec![0; (size * size * 4) as usize];
    let s = size as f32;
    let center = s / 2.0;
    let (outer, inner, strut, core) = (44.0f32, 33.0, 3.0, 11.0);
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let distance = dx.hypot(dy);
            let color: Option<[u8; 3]> = if distance <= core {
                Some([95, 230, 220])
            } else if (inner..=outer).contains(&distance) {
                if distance > outer - 2.0 || distance < inner + 2.0 {
                    Some([70, 80, 100])
                } else {
                    Some([150, 168, 192])
                }
            } else if distance <= outer && (dx.abs() <= strut || dy.abs() <= strut) {
                Some([110, 124, 150])
            } else {
                None
            };
            if let Some(color) = color {
                let index = ((y * size + x) * 4) as usize;
                data[index..index + 3].copy_from_slice(&color);
                data[index + 3] = 255;
            }
        }
    }
    for k in 0..12u32 {
        if k % 4 == 3 {
            continue;
        }
        let angle = k as f32 / 12.0 * std::f32::consts::TAU;
        let ring = (inner + outer) / 2.0;
        let x = (center + ring * angle.cos()) as i32;
        let y = (center + ring * angle.sin()) as i32;
        let window = if k % 2 == 0 {
            [120, 240, 230]
        } else {
            [255, 220, 120]
        };
        let index = ((y * size as i32 + x) * 4) as usize;
        data[index..index + 3].copy_from_slice(&window);
    }
    outline_silhouette(&mut data, size, [21, 24, 32]);
    image_from_rgba(size, data)
}

fn star_image(size: u32) -> Image {
    let mut data = vec![0; (size * size * 4) as usize];
    let s = size as f32;
    let disc = s * 0.30;
    let corona = s * 0.48;
    let core = [255, 244, 214];
    let bands: [[u8; 4]; 4] = [
        [255, 196, 110, 235],
        [255, 140, 60, 160],
        [255, 96, 46, 95],
        [210, 60, 40, 45],
    ];
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - s / 2.0;
            let dy = y as f32 + 0.5 - s / 2.0;
            let distance = (dx * dx + dy * dy).sqrt();
            let index = ((y * size + x) * 4) as usize;
            if distance <= disc {
                data[index..index + 4].copy_from_slice(&[core[0], core[1], core[2], 255]);
            } else if distance <= corona {
                let t = 1.0 - (distance - disc) / (corona - disc);
                let level = (t * 4.0 + bayer(x as i32, y as i32))
                    .floor()
                    .clamp(0.0, 4.0) as usize;
                if level > 0 {
                    data[index..index + 4].copy_from_slice(&bands[level - 1]);
                }
            }
        }
    }
    image_from_rgba(size, data)
}

fn black_hole_image(size: u32) -> Image {
    let mut data = vec![0; (size * size * 4) as usize];
    let s = size as f32;
    let rings: [[u8; 4]; 3] = [
        [210, 110, 245, 235],
        [150, 70, 200, 170],
        [85, 35, 130, 100],
    ];
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 + 0.5) / s - 0.5;
            let dy = (y as f32 + 0.5) / s - 0.5;
            let distance = dx.hypot(dy);
            let index = ((y * size + x) * 4) as usize;
            if distance <= 0.25 {
                data[index..index + 4].copy_from_slice(&[2, 1, 6, 255]);
            } else if distance <= 0.48 {
                let ring = 1.0 - ((distance - 0.32) / 0.16).abs();
                if ring > 0.0 {
                    let level = (ring * 3.0 + bayer(x as i32, y as i32))
                        .floor()
                        .clamp(0.0, 2.0) as usize;
                    let shimmer = 0.7 + 0.3 * (dy.atan2(dx) * 3.0).sin();
                    let band = rings[level];
                    data[index..index + 4].copy_from_slice(&[
                        band[0],
                        band[1],
                        band[2],
                        (band[3] as f32 * shimmer) as u8,
                    ]);
                }
            }
        }
    }
    image_from_rgba(size, data)
}

fn glow_image(size: u32) -> Image {
    let mut data = vec![0; (size * size * 4) as usize];
    let s = size as f32;
    let radius = s / 2.0;
    let bands: [(u8, u8); 4] = [(60, 140), (130, 200), (200, 235), (255, 255)];
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - radius;
            let dy = y as f32 + 0.5 - radius;
            let t = 1.0 - (dx * dx + dy * dy).sqrt() / radius;
            if t > 0.0 {
                let level = (t * 4.0 + bayer(x as i32, y as i32))
                    .floor()
                    .clamp(0.0, 4.0) as usize;
                let (value, alpha) = bands[level.min(3)];
                let index = ((y * size + x) * 4) as usize;
                data[index] = value;
                data[index + 1] = value;
                data[index + 2] = value;
                data[index + 3] = alpha;
            }
        }
    }
    image_from_rgba(size, data)
}

fn nebula_image(size: u32) -> Image {
    let mut data = vec![0; (size * size * 4) as usize];
    let s = size as f32;
    let radius = s / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - radius;
            let dy = y as f32 + 0.5 - radius;
            let t = 1.0 - (dx * dx + dy * dy).sqrt() / radius;
            if t > 0.0 {
                let level = (t * 6.0 + bayer(x as i32, y as i32))
                    .floor()
                    .clamp(0.0, 6.0) as usize;
                let index = ((y * size + x) * 4) as usize;
                data[index] = (42 * level as u32).min(255) as u8;
                data[index + 1] = (42 * level as u32).min(255) as u8;
                data[index + 2] = (42 * level as u32).min(255) as u8;
                data[index + 3] = (36 * level as u32).min(216) as u8;
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

const BAYER4: [[u32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

fn bayer(x: i32, y: i32) -> f32 {
    BAYER4[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize] as f32 / 16.0
}

fn quantize_shading(data: &mut [u8], size: u32, steps: u32) {
    for y in 0..size {
        for x in 0..size {
            let index = ((y * size + x) * 4) as usize;
            if data[index + 3] == 0 {
                continue;
            }
            let offset = bayer(x as i32, y as i32);
            for channel in 0..3 {
                let value = data[index + channel] as f32 / 255.0;
                let level = (value * steps as f32 + offset).floor();
                data[index + channel] = ((level / steps as f32) * 255.0).clamp(0.0, 255.0) as u8;
            }
        }
    }
}

fn outline_silhouette(data: &mut [u8], size: u32, color: [u8; 3]) {
    let mut solid = vec![false; (size * size) as usize];
    for y in 0..size {
        for x in 0..size {
            let index = ((y * size + x) * 4) as usize;
            solid[(y * size + x) as usize] = data[index + 3] > 0;
        }
    }
    for y in 0..size {
        for x in 0..size {
            let cell = (y * size + x) as usize;
            if !solid[cell] {
                continue;
            }
            let edge = x == 0
                || y == 0
                || x + 1 == size
                || y + 1 == size
                || !solid[cell - 1]
                || !solid[cell + 1]
                || !solid[cell - size as usize]
                || !solid[cell + size as usize];
            if edge {
                let index = cell * 4;
                data[index..index + 3].copy_from_slice(&color);
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_opaque_pixels(image: &Image, min_count: usize) {
        let data = image.data.as_ref().expect("texture keeps cpu data");
        let opaque = data
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|chunk| chunk[3] > 0)
            .count();
        assert!(opaque >= min_count, "expected at least {min_count} pixels");
    }

    #[test]
    fn procedural_textures_build() {
        for size in [36, 48, 68, 92] {
            let ship = ship_image(size);
            assert_eq!(ship.width(), size);
            assert_opaque_pixels(&ship, size as usize * size as usize / 10);
        }
        assert_opaque_pixels(&planet_image(180), 19_000);
        assert_opaque_pixels(&moon_image(64), 2_000);
        assert_opaque_pixels(&planet_ring_half_image(192, false), 1_500);
        assert_opaque_pixels(&planet_ring_half_image(192, true), 1_500);
        assert_opaque_pixels(&station_image(128), 1_000);
        assert_opaque_pixels(&star_image(128), 3_000);
        assert_opaque_pixels(&black_hole_image(128), 1_000);
        assert_opaque_pixels(&glow_image(16), 100);
        assert_opaque_pixels(&nebula_image(64), 1_000);
        assert_opaque_pixels(&chief_image(128), 3_000);
        assert_opaque_pixels(&meteorite_image(62, 0), 1_000);
        assert_opaque_pixels(&meteorite_image(38, 1), 400);
        assert_opaque_pixels(&meteorite_image(22, 2), 150);
        let chief = chief_image(128);
        assert!(matches!(chief.sampler, ImageSampler::Descriptor(_)));
    }
}
