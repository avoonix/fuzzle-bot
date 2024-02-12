use anyhow::Result;
use base64::{engine::general_purpose, Engine};
use blake2::{digest::consts::U16, Blake2b, Digest};
use itertools::Itertools;

use super::download::FileKind;

type Blake2b128 = Blake2b<U16>;

pub fn calculate_visual_hash(buf: Vec<u8>, file_kind: FileKind) -> Result<Option<String>> {
    let visual_hash = match file_kind {
        FileKind::Image => {
            let dynamic_image = image::load_from_memory(&buf)?;
            let dynamic_image =
                dynamic_image.resize_exact(32, 32, image::imageops::FilterType::Gaussian);

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
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|x| x.abs())
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap()
                })
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap();
            let dct = dct
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|value| ((value / max * 16.0).round()).min(15.0) as u8)
                        .collect_vec()
                })
                .collect_vec();

            let snake = (0..8)
                .flat_map(|i| (0..=i).map(move |j| (i - j, j)))
                .collect_vec();

            let values = snake.iter().map(|(x, y)| dct[*y][*x]).collect_vec();
            let result = u4_to_u8(values);

            Some(general_purpose::URL_SAFE_NO_PAD.encode(result))
        }
        FileKind::Unknown => None,
    };

    Ok(visual_hash)
}

pub fn calculate_sticker_file_hash(buf: Vec<u8>, file_kind: FileKind) -> Result<String> {
    let hash = Blake2b128::digest(&buf);
    Ok(general_purpose::URL_SAFE_NO_PAD.encode(hash))
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

fn u4_to_u8(input: Vec<u8>) -> Vec<u8> {
    input
        .iter()
        .rev()
        .chunks(2)
        .into_iter()
        .map(|chunk| {
            let mut value = 0;
            for (i, x) in chunk.enumerate() {
                value |= x << (i * 4);
            }
            value
        })
        .collect_vec()
        .into_iter()
        .rev()
        .collect_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u4_to_u8() {
        let input = vec![0, 1, 2, 3, 4, 5, 6, 7];
        let output = u4_to_u8(input);
        assert_eq!(output, vec![0x01, 0x23, 0x45, 0x67]);
    }
}
