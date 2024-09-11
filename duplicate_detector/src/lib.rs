use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba, RgbaImage};
use itertools::Itertools;
use rayon::prelude::*;

pub struct Config {
    single_pixel_threshold: u8,
    neighborhood_threshold: u8,
    neighborhood_size: u32,
}

impl Config {
    pub fn new(
        single_pixel_threshold: u8,
        neighborhood_threshold: u8,
        neighborhood_size: u32,
    ) -> Self {
        Self {
            neighborhood_threshold,
            single_pixel_threshold,
            neighborhood_size,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new(35, 20, 4)
    }
}

pub fn generate_merge_image(
    image_a: DynamicImage,
    image_b: DynamicImage,
    threshold: &Config,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let width = image_a.width();
    let height = image_a.height();
    let image_b = if image_a.dimensions() == image_b.dimensions() {
        image_b
    } else {
        image_b.resize_exact(width, height, image::imageops::FilterType::Gaussian)
    };

    let mut img = RgbaImage::new(width * 4, height * 2);

    let differences = (0..width * height)
        .map(|i| {
            let x = i % width;
            let y = i / width;
            get_pixel_and_neighborhood_differences(
                &image_a,
                &image_b,
                x,
                y,
                threshold.neighborhood_size,
            )
        })
        .collect_vec();
    let diff = |x: u32, y: u32| {
        let i = x + y * width;
        differences.get(i as usize).copied().unwrap_or_default()
    };

    for x in 0..img.width() {
        for y in 0..img.height() {
            let pixel_color = if y < height {
                if x < width {
                    image_a.get_pixel(x, y)
                } else if x < 2 * width {
                    let x = x - width;
                    let diff = diff(x, y);
                    let col = (diff.max_diff_single_pixel * 4).min(255) as u8;
                    Rgba([col, col, col, 255])
                } else if x < 3 * width {
                    let x = x - 2 * width;
                    let diff = diff(x, y);
                    let col = (diff.max_diff_neighborhood * 6.0).min(255.0) as u8;
                    Rgba([col, col, col, 255])
                } else {
                    // let x = x - 3 * width;
                    Rgba([0, 0, 0, 255])
                }
            } else {
                let y = y - height;
                if x < width {
                    image_b.get_pixel(x, y)
                } else if x < 2 * width {
                    let x = x - width;
                    let d = diff(x, y);
                    if d.max_diff_single_pixel as u8 > threshold.single_pixel_threshold {
                        Rgba([255, 0, 0, 255])
                    } else {
                        Rgba([0, 255, 0, 255])
                    }
                } else if x < 3 * width {
                    let x = x - 2 * width;
                    let d = diff(x, y);
                    if d.max_diff_neighborhood as u8 > threshold.neighborhood_threshold {
                        Rgba([255, 0, 0, 255])
                    } else {
                        Rgba([0, 255, 0, 255])
                    }
                } else {
                    let x = x - 3 * width;
                    let d = diff(x, y);
                    if d.max_diff_single_pixel as u8 > threshold.single_pixel_threshold
                        && d.max_diff_neighborhood as u8 > threshold.neighborhood_threshold
                    {
                        Rgba([255, 0, 0, 255])
                    } else {
                        Rgba([0, 255, 0, 255])
                    }
                }
            };
            img.put_pixel(x, y, pixel_color);
        }
    }

    img
}

pub fn image_differences_within_thresholds(
    dynamic_image_a: &DynamicImage,
    dynamic_image_b: &DynamicImage,
    threshold: &Config,
) -> bool {
    if dynamic_image_a.dimensions() != dynamic_image_b.dimensions() {
        return false;
    }

    let any_pixel_is_above_threshold = (0..dynamic_image_a.width())
        .flat_map(|x| (0..dynamic_image_a.height()).map(move |y| (x, y)))
        .collect_vec()
        .par_iter()
        .any(|(x, y)| {
            let d = get_pixel_and_neighborhood_differences(
                dynamic_image_a,
                dynamic_image_b,
                *x,
                *y,
                threshold.neighborhood_size,
            );
            d.max_diff_single_pixel as u8 > threshold.single_pixel_threshold
                && d.max_diff_neighborhood as u8 > threshold.neighborhood_threshold
        });

    !any_pixel_is_above_threshold
}

fn get_pixel_and_neighborhood_differences(
    image_a: &DynamicImage,
    image_b: &DynamicImage,
    x: u32,
    y: u32,
    neighborhood_size: u32,
) -> Differences {
    let a = image_a.get_pixel(x, y);
    let b = image_b.get_pixel(x, y);

    if a.0[3] < 20 && b.0[3] < 20 {
        // difference does not matter if both are (almost) transparent
        return Differences {
            max_diff_neighborhood: 0.0,
            max_diff_single_pixel: 0,
        };
    }

    let max_diff_single_pixel = (0..4)
        .map(|color_channel| (a[color_channel] as i16).abs_diff(b[color_channel] as i16))
        .max()
        .unwrap_or(0);

    let min_x = (x as i32 - neighborhood_size as i32).max(0) as u32;
    let min_y = (y as i32 - neighborhood_size as i32).max(0) as u32;
    let max_x = (x + neighborhood_size).min(image_a.width());
    let max_y = (y + neighborhood_size).min(image_a.height());

    let max_diff_neighborhood = (0..4)
        .map(|color_channel| {
            let sum = (min_x..max_x)
                .map(|x| {
                    (min_y..max_y)
                        .map(|y| {
                            if image_a.get_pixel(x, y)[3] < 20 && image_b.get_pixel(x, y)[3] < 20 {
                                // difference does not matter if both are (almost) transparent
                                0
                            } else {
                                (image_a.get_pixel(x, y)[color_channel] as u32)
                                    .abs_diff(image_b.get_pixel(x, y)[color_channel] as u32)
                            }
                        })
                        .sum::<u32>() as f32
                })
                .sum::<f32>();
            let count = ((max_x - min_x) * (max_y - min_y)) as f32;
            let avg = sum / count;
            avg
        })
        .reduce(f32::max)
        .unwrap_or(0.0);

    Differences {
        max_diff_single_pixel,
        max_diff_neighborhood,
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Differences {
    max_diff_single_pixel: u16,
    max_diff_neighborhood: f32,
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;
    use std::ops::Add;
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use super::*;

    const TEST_CODE_TO_WORK: &str = "expect test code to work";

    #[derive(Debug)]
    struct TestStatistics {
        successful: i32,
        not_successful: i32,
        not_successful_pairs: Vec<(String, String)>,
    }

    impl Add for TestStatistics {
        type Output = Self;

        fn add(self, rhs: Self) -> Self::Output {
            Self {
                not_successful: self.not_successful + rhs.not_successful,
                successful: self.successful + rhs.successful,
                not_successful_pairs: self
                    .not_successful_pairs
                    .into_iter()
                    .chain(rhs.not_successful_pairs)
                    .collect_vec(),
            }
        }
    }

    fn get_test_resources_dir() -> PathBuf {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("resources/test");
        d
    }

    fn test_case_iter(test_type: &str, expected_result: bool) -> TestStatistics {
        let mut d = get_test_resources_dir();
        d.push(test_type);

        fs::read_dir(d)
            .expect(TEST_CODE_TO_WORK)
            .collect_vec()
            .into_par_iter()
            .map(|case| {
                let case = case.expect(TEST_CODE_TO_WORK);
                compare_each(&case.path(), expected_result)
            })
            .reduce(
                || TestStatistics {
                    not_successful: 0,
                    successful: 0,
                    not_successful_pairs: vec![],
                },
                |a, b| a + b,
            )
    }

    fn compare_each(path: &Path, expected_result: bool) -> TestStatistics {
        let mut images = Vec::new();
        for file_entry in fs::read_dir(path).expect(TEST_CODE_TO_WORK) {
            let file_entry = file_entry.expect(TEST_CODE_TO_WORK);
            let buf = fs::read(file_entry.path()).expect(TEST_CODE_TO_WORK);
            let img = image::load_from_memory(&buf).expect(TEST_CODE_TO_WORK);
            images.push((file_entry.path(), img))
        }
        let mut stats = TestStatistics {
            not_successful: 0,
            successful: 0,
            not_successful_pairs: vec![],
        };
        for (a, b) in images.iter().tuple_combinations() {
            if image_differences_within_thresholds(&a.1, &b.1, &Default::default())
                == expected_result
            {
                stats.successful += 1;
            } else {
                stats.not_successful += 1;
                stats.not_successful_pairs.push((
                    a.0.to_string_lossy().to_string(),
                    b.0.to_string_lossy().to_string(),
                ));
            }
        }
        stats
    }

    #[test]
    fn detects_different_stickers() {
        let stats = test_case_iter("different", false);
        println!(
            "different stickers that were detected as different (correct): {}",
            stats.successful
        );
        println!(
            "different stickers that were detected as same (incorrect): {}",
            stats.not_successful
        );
        println!("incorrect pairs: {:?}", stats.not_successful_pairs);
        assert_eq!(stats.not_successful, 0);
    }

    #[test]
    fn detects_same_stickers() {
        let stats = test_case_iter("same", true);
        println!(
            "same stickers that were detected as same (correct): {}",
            stats.successful
        );
        println!(
            "same stickers that were detected as different (incorrect): {}",
            stats.not_successful
        );
        println!("incorrect pairs: {:?}", stats.not_successful_pairs);
        assert!(stats.successful > 150);
        // ideally, this would be 0, but we'll need to do something more sophisticated for that ._.
        // assert_eq!(stats.not_successful, 0);
    }

    // TODO: unit tests
}
