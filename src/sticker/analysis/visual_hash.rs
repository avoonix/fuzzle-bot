use crate::sticker::download::FileKind;
use base64::{engine::general_purpose, Engine};
use itertools::Itertools;
use std::{collections::HashMap, io::Cursor};
use image::{Rgb, RgbImage, Rgba, RgbaImage};
use palette::{Hsv, IntoColor, Srgb};



#[derive(Debug)]
pub struct VisualHash {
    normalized_vec: Vec<u8>,
}

impl From<VisualHash> for Vec<u8> {
    fn from(value: VisualHash) -> Self {
        value.normalized_vec
    }
}

impl From<Vec<u8>> for VisualHash {
    fn from(value: Vec<u8>) -> Self {
        VisualHash {
            normalized_vec: value,
        }
    }
}

pub fn calculate_visual_hash(buf: Vec<u8>) -> anyhow::Result<VisualHash> {
    let dynamic_image = image::load_from_memory(&buf)?;
    let dynamic_image = dynamic_image.resize_exact(32, 32, image::imageops::FilterType::Gaussian);

    let mut image = dynamic_image.into_rgba8();
    let mut gray = vec![vec![0; image.width() as usize]; image.height() as usize];
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let r = f64::from(pixel.0[0]) / 255.0;
        let g = f64::from(pixel.0[1]) / 255.0;
        let b = f64::from(pixel.0[2]) / 255.0;

        gray[y as usize][x as usize] =
            (0.0722f64.mul_add(b, 0.2126f64.mul_add(r, 0.7152 * g)) * 255.0) as u8;
    }
    let dct = two_dimensional_dct(gray);
    let max = dct
        .clone()
        .into_iter()
        .map(|row| row.into_iter().reduce(f64::max).unwrap_or_default())
        .reduce(f64::max)
        .unwrap_or_default();
    let dct = dct
        .iter()
        .map(|row| {
            row.iter()
                .map(|value| ((value / max * 255.0).round()).min(255.0) as u8)
                .collect_vec()
        })
        .collect_vec();

    let snake = (0..8)
        .flat_map(|i| (0..=i).map(move |j| (i - j, j)))
        .collect_vec();

    Ok(snake.iter().map(|(x, y)| dct[*y][*x]).collect_vec().into())
}


fn transpose<T>(v: Vec<Vec<T>>) -> Vec<Vec<T>>
where
    T: Clone,
{
    assert!(!v.is_empty());
    (0..v[0].len())
        .map(|i| v.iter().map(|inner| inner[i].clone()).collect::<Vec<T>>())
        .collect()
}

fn two_dimensional_dct(input: Vec<Vec<u8>>) -> Vec<Vec<f64>> {
    let mut input = input;
    let mut output = vec![vec![0.0; input[0].len()]; input.len()];
    let mut dct = rustdct::DctPlanner::new();
    let transform = dct.plan_dct2(input[0].len());
    for (i, row) in input.iter_mut().enumerate() {
        output[i] = row
            .clone()
            .into_iter()
            .map(|x| f64::from(x) - 128.0)
            .collect_vec();
        transform.process_dct2(&mut output[i]);
    }

    let mut output = transpose(output);

    // dct on columns
    for row in &mut output {
        transform.process_dct2(row);
    }

    transpose(output)
}

pub fn create_visual_hash_image(visual_hash: VisualHash) -> anyhow::Result<Vec<u8>> {
    let mut img = RgbaImage::new(visual_hash.normalized_vec.len() as u32, 256);

    for x in 0..img.width() {
        for y in 0..img.height() {
            let hash_entry = visual_hash.normalized_vec[x as usize];
            let pixel_color = if img.height() - y <= hash_entry as u32  {
                Rgba([
                    hash_entry,
                    hash_entry,
                    hash_entry,
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
