use base64::{engine::general_purpose, Engine};
use once_cell::sync::Lazy;
use std::{collections::HashMap, path::PathBuf, sync::{Mutex, PoisonError}};
use teloxide::{net::Download, requests::Requester};
use tokio::{
    fs::{create_dir_all, try_exists, File},
    io::{AsyncReadExt, AsyncWriteExt},
};

use crate::bot::{Bot, BotError};

#[derive(Debug, Clone, Copy)]
pub enum FileKind {
    Image,
    Unknown,
    // TODO: parse other file types (video, tgs)
}

impl From<&str> for FileKind {
    fn from(file: &str) -> Self {
        if file.ends_with(".webp") || file.ends_with(".jpeg") || file.ends_with(".jpg") {
            // some sticker packs apparently have png stickers desipite .webp file extension???
            Self::Image
        } else {
            Self::Unknown
            // Err(anyhow::anyhow!("unknown file type"))
        }
    }
}

/// TODO: refactor
static CACHED_STICKER_FILE: Lazy<Mutex<HashMap<PathBuf, Vec<u8>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// memory/disk cache
pub async fn fetch_possibly_cached_sticker_file(
    file_id: String,
    bot: Bot,
    image_cache_path: PathBuf,
) -> Result<Vec<u8>, BotError> {
    let filename = general_purpose::URL_SAFE_NO_PAD.encode(file_id.clone());
    let path = image_cache_path.join(filename);
    if let Some(buf) = CACHED_STICKER_FILE.lock().unwrap_or_else(PoisonError::into_inner).get(&path) {
        return Ok(buf.clone());
    }

    let exists = try_exists(path.clone()).await?;
    let buf = if exists {
        let mut file = File::open(path.clone()).await?;
        let mut contents = vec![];
        file.read_to_end(&mut contents).await?;

        contents
    } else {
        let (buf, file) = fetch_sticker_file(file_id, bot).await?;
        if let Some(parent) = path.parent() {
            create_dir_all(parent).await?;
        }
        let mut file = File::create(path.clone()).await?;
        file.write_all(&buf).await?;

        buf
    };
    CACHED_STICKER_FILE
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(path, buf.clone());
    Ok(buf)
}

pub async fn fetch_sticker_file(
    file_id: String,
    bot: Bot,
) -> Result<(Vec<u8>, teloxide::types::File), BotError> {
    let file = bot.get_file(file_id).await?;
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
