use std::{collections::HashMap, io::Cursor};

use image::{Rgb, RgbImage, Rgba, RgbaImage};
use itertools::Itertools;
use palette::{Hsv, IntoColor, Srgb};

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
        Histogram {
            normalized_vec: value,
        }
    }
}

impl Histogram {
    fn from_map(map: HashMap<(u8, u8, u8), u32>) -> Histogram {
        let max = {
            let max = map.values().max().cloned().unwrap_or_default() as f32; // TODO: make sure this is > 0 because of division
            if max == 0.0 {
                1.0
            } else {
                max
            }
        };

        let all_colors = all_colors();

        let normalized_vec = all_colors
            .into_iter()
            .map(|color| {
                ((map.get(&color).cloned().unwrap_or_default() as f32 / max) * 255.0) as u8
            })
            .collect();

        Histogram { normalized_vec }
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
                map.insert(color, value as f32 / 255.0);
            }
        }
        map
    }
}

// TODO: no anyhow
pub fn calculate_color_histogram(buf: Vec<u8>) -> anyhow::Result<Histogram> {
    let dynamic_image = image::load_from_memory(&buf)?;
    let mut image = dynamic_image.into_rgba8();
    let mut colors = HashMap::new();
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let r = ((pixel.0[0] as u32 * BINS) / 255) as u8;
        let g = ((pixel.0[1] as u32 * BINS) / 255) as u8;
        let b = ((pixel.0[2] as u32 * BINS) / 255) as u8;
        let entry: &mut u32 = colors.entry((r, g, b)).or_default();
        *entry += pixel.0[3] as u32; // more opaque = higher weight
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
                (a.0 * scale) as f32 / 255.0,
                (a.1 * scale) as f32 / 255.0,
                (a.2 * scale) as f32 / 255.0,
            );
            let hsl_a: Hsv = rgb.into_color();
            let rgb = Srgb::new(b.0 as f32 / 255.0, b.1 as f32 / 255.0, b.2 as f32 / 255.0);
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
            let value = map.get(&hist_entry).cloned().unwrap_or_default();
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
    img.write_to(&mut Cursor::new(&mut bytes), image::ImageOutputFormat::WebP)?;

    Ok(bytes)
}
