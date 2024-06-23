use std::{collections::HashMap, io::Cursor};

use image::{Rgba, RgbaImage};
use itertools::Itertools;
use palette::{Hsv, IntoColor, Srgb};

use crate::bot::InternalError;

const BINS: u32 = 5; // per color channel

#[derive(Debug)]
pub struct Histogram {
    normalized_vec: Vec<u8>,
}

impl From<Histogram> for Vec<u8> {
    fn from(value: Histogram) -> Self {
        value.normalized_vec
    }
}

impl From<Vec<u8>> for Histogram {
    fn from(value: Vec<u8>) -> Self {
        Self {
            normalized_vec: value,
        }
    }
}

impl Histogram {
    fn from_map(map: HashMap<(u8, u8, u8), u32>) -> Self {
        let all_colors = all_colors();

        let max = map.values().max().copied().unwrap_or_default() as f32;
        if max < 1.0 {
            // empty image; cosine similarity is undefined for 0
            return Self { normalized_vec: all_colors.into_iter().with_position().map(|(pos, _)| match pos {
                itertools::Position::First | itertools::Position::Only => 1,
                itertools::Position::Last | itertools::Position::Middle => 0,
            }).collect_vec() };
        }


        let normalized_vec = all_colors
            .into_iter()
            .map(|color| {
                ((map.get(&color).copied().unwrap_or_default() as f32 / max) * 255.0) as u8
            })
            .collect();

        Self { normalized_vec }
    }

    fn get_map(self) -> HashMap<(u8, u8, u8), f32> {
        let mut map = HashMap::new();
        for (i, (color, value)) in all_colors()
            .into_iter()
            .zip(self.normalized_vec.into_iter())
            .enumerate()
        {
            if value != 0 {
                // TODO: approximate match?
                map.insert(color, f32::from(value) / 255.0);
            }
        }
        map
    }
}

pub fn calculate_color_histogram(buf: Vec<u8>) -> Result<Histogram, InternalError> {
    let dynamic_image = image::load_from_memory(&buf)?;
    let mut image = dynamic_image.into_rgba8();
    let mut colors = HashMap::new();
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let r = ((u32::from(pixel.0[0]) * BINS) / 256) as u8;
        let g = ((u32::from(pixel.0[1]) * BINS) / 256) as u8;
        let b = ((u32::from(pixel.0[2]) * BINS) / 256) as u8;
        let entry: &mut u32 = colors.entry((r, g, b)).or_default();
        *entry += u32::from(pixel.0[3]); // more opaque = higher weight
    }

    Ok(Histogram::from_map(colors))
}

#[cached::proc_macro::once]
fn all_colors() -> Vec<(u8, u8, u8)> {
    (0..(BINS as u8))
        .flat_map(move |r| {
            (0..(BINS as u8)).flat_map(move |g| (0..(BINS as u8)).map(move |b| (r, g, b)))
        })
        .collect()
}

pub fn create_historgram_image(histogram: Histogram) -> anyhow::Result<Vec<u8>> {
    let width = BINS * BINS * BINS;
    let height = width / 2;
    let mut img = RgbaImage::new(width, height);
    let scale = (256 / BINS) as u8;

    let all_colors = all_colors()
        .into_iter()
        .sorted_by(|a, b| {
            let rgb = Srgb::new(
                f32::from(a.0 * scale) / 255.0,
                f32::from(a.1 * scale) / 255.0,
                f32::from(a.2 * scale) / 255.0,
            );
            let hsl_a: Hsv = rgb.into_color();
            let rgb = Srgb::new(f32::from(b.0) / 255.0, f32::from(b.1) / 255.0, f32::from(b.2) / 255.0);
            let hsl_b: Hsv = rgb.into_color();

            let a = hsl_a.hue.into_positive_degrees();
            let b = hsl_b.hue.into_positive_degrees();

            a.total_cmp(&b).then(hsl_a.value.total_cmp(&hsl_b.value))
        })
        .collect_vec();

    let max_height = (height as f32).log10();
    let map = histogram.get_map();

    for x in 0..img.width() {
        for y in 0..img.height() {
            let hist_entry = all_colors[x as usize];
            let value = map.get(&hist_entry).copied().unwrap_or_default();
            let height = (value * height as f32).log10() / max_height;
            let pixel_color = if 1.0 - y as f32 / img.height() as f32 <= height {
                Rgba([
                    hist_entry.0 * scale,
                    hist_entry.1 * scale,
                    hist_entry.2 * scale,
                    255,
                ])
            } else {
                Rgba([255, 255, 255, 0])
            };
            img.put_pixel(x, y, pixel_color);
        }
    }

    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::WebP)?;

    Ok(bytes)
}
