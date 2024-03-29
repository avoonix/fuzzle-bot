use flate2::read::GzDecoder;
use image::{io::Reader as ImageReader, Pixel};
use once_cell::sync::Lazy;
use std::{io::Cursor};
use tokio::{io::AsyncReadExt};
use tract_itertools::Itertools;
use tract_onnx::prelude::*;
use thiserror::Error;

use super::tokenizer::tokenize;

#[derive(Error, Debug)]
pub enum EmbeddingError {
    #[error("unknown token")]
    UnknownToken(String),

    #[error("io error")]
    Io(#[from] std::io::Error),

    #[error("image decoding error")]
    Image(#[from] image::ImageError),

    #[error("other error")]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, Clone)]
pub struct ModelEmbedding {
    vec: Vec<f32>,
}

impl From<ModelEmbedding> for Vec<u8> {
    fn from(value: ModelEmbedding) -> Self {
        value
            .vec
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect()
    }
}

impl From<Vec<u8>> for ModelEmbedding {
    fn from(value: Vec<u8>) -> Self {
        Self {
            vec: value
                .chunks_exact(4)
                .map(|val| {
                    let val: [u8; 4] = val.try_into().unwrap();
                    f32::from_le_bytes(val)
                })
                .collect(),
        }
    }
}

impl From<ModelEmbedding> for Vec<f32> {
    fn from(value: ModelEmbedding) -> Self {
        value.vec
    }
}

impl From<ModelEmbedding> for Vec<f64> {
    fn from(value: ModelEmbedding) -> Self {
        value.vec.into_iter().map(f64::from).collect()
    }
}

impl From<Vec<f32>> for ModelEmbedding {
    fn from(value: Vec<f32>) -> Self {
        Self { vec: value }
    }
}

static TEXTUAL_MODEL: Lazy<
    SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>,
> = Lazy::new(|| {
    let model1 = include_bytes!("clip/textual.1.onnx.gz");
    let model2 = include_bytes!("clip/textual.2.onnx.gz");
    let model = model1.iter().chain(model2.iter()).copied().collect_vec();
    let mut gz = GzDecoder::new(model.as_slice());
    onnx()
        .model_for_read(&mut gz)
        .expect("hardcoded model to work")
        .into_optimized()
        .expect("hardcoded model to work")
        .into_runnable()
        .expect("hardcoded model to work")
});

static VISUAL_MODEL: Lazy<
    SimplePlan<TypedFact, Box<dyn TypedOp>, Graph<TypedFact, Box<dyn TypedOp>>>,
> = Lazy::new(|| {
    let model1 = include_bytes!("clip/visual.1.onnx.gz");
    let model2 = include_bytes!("clip/visual.2.onnx.gz");
    let model = model1.iter().chain(model2.iter()).copied().collect_vec();
    let mut gz = GzDecoder::new(model.as_slice());
    onnx()
        .model_for_read(&mut gz)
        .expect("hardcoded model to work")
        .into_optimized()
        .expect("hardcoded model to work")
        .into_runnable()
        .expect("hardcoded model to work")
});

impl ModelEmbedding {
    pub fn from_text(text: &str) -> Result<Self, EmbeddingError> {
        let model = &TEXTUAL_MODEL;
        let tokens = tokenize(&[text.to_string()])?;
        let result = model.run(tvec!(tokens.into()))?;
        Ok(result[0]
            .to_array_view::<f32>()?
            .iter()
            .copied()
            .map(|f| f * 100.0)
            .collect_vec()
            .into())
    }

    pub fn from_image_buf(buf: Vec<u8>) -> Result<Self, EmbeddingError> {
        let model = &VISUAL_MODEL;
        let img2 = ImageReader::new(Cursor::new(buf))
            .with_guessed_format()?
            .decode()?;
        let image = img2.to_rgb8();
        let resized =
            image::imageops::resize(&image, 224, 224, ::image::imageops::FilterType::Triangle); // TODO: should be bicubic
        let image: Tensor =
            tract_ndarray::Array4::from_shape_fn((1, 3, 224, 224), |(_, c, y, x)| {
                // https://github.com/openai/CLIP/blob/a1d071733d7111c9c014f024669f959182114e33/clip/clip.py#L85
                let mean = [0.481_454_66, 0.457_827_5, 0.408_210_73][c];
                let std = [0.268_629_54, 0.261_302_6, 0.275_777_1][c];
                (f32::from(resized[(x as _, y as _)][c]) / 255.0 - mean) / std
            })
            .into();
        let result = model.run(tvec!(image.into()))?;
        let vec = result[0]
            .to_array_view::<f32>()?
            .iter()
            .copied()
            .map(|f| f * 100.0)
            .collect_vec();

        Ok(vec.into())
    }
}
