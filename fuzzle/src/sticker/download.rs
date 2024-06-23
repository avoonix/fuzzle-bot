use base64::{engine::general_purpose, Engine};
use once_cell::sync::Lazy;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Mutex, PoisonError},
};
use teloxide::{net::Download, requests::Requester};
use tokio::{
    fs::{create_dir_all, try_exists, File},
    io::{AsyncReadExt, AsyncWriteExt},
};

use crate::bot::{Bot, BotError, InternalError};

#[derive(Debug, Clone, Copy)]
pub enum FileKind {
    Image,
    Tgs,
    Unknown,
    // TODO: parse other file types (video, tgs)
}

impl From<&str> for FileKind {
    fn from(file: &str) -> Self {
        if file.ends_with(".webp")
            || file.ends_with(".jpeg")
            || file.ends_with(".jpg")
            || file.ends_with(".png")
        {
            // some sticker packs apparently have png stickers desipite .webp file extension???
            Self::Image
        } else if file.ends_with(".tgs") {
            Self::Tgs
        } else {
            Self::Unknown
            // Err(anyhow::anyhow!("unknown file type"))
        }
    }
}

#[tracing::instrument(skip(bot))]
pub async fn fetch_sticker_file(
    file_id: String,
    bot: Bot,
) -> Result<(Vec<u8>, teloxide::types::File), InternalError> {
    let file = bot.get_file(file_id).await?;
    tracing::info!(file.path = file.path);
    let mut buf = Vec::new();
    bot.download_file(&file.path, &mut buf).await?;
    if buf.len() == file.size as usize {
        Ok((buf, file))
    } else {
        Err(anyhow::anyhow!(
            "file size mismatch: {} != {}",
            buf.len(),
            file.size
        ))?
    }
}
