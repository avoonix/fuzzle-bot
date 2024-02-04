use teloxide::{net::Download, requests::Requester};

use crate::bot::{Bot, BotError};

#[derive(Debug, Clone, Copy)]
pub enum FileKind {
    WebpOrPng,
    Unknown,
    // TODO: parse other file types (video, tgs)
}

impl From<&str> for FileKind {
    fn from(file: &str) -> Self {
        if file.ends_with(".webp") {
            // some sticker packs apparently have png stickers desipite .webp file extension???
            Self::WebpOrPng
        } else {
            Self::Unknown
            // Err(anyhow::anyhow!("unknown file type"))
        }
    }
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
