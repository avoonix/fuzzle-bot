use std::{io::Cursor, sync::Arc};

use futures::{stream::FuturesUnordered, FutureExt};
use image::{GenericImage, ImageBuffer, Rgba};
use itertools::Itertools;

use crate::{bot::{Bot, BotError}, database::StickerFile, Config};

use super::fetch_sticker_file;


#[tracing::instrument(skip(stickers, size, bot, config), err(Debug))]
pub async fn create_sticker_thumbnail(
    stickers: Vec<StickerFile>,
    size: u32,
    bot: Bot,
    config: Arc<Config>,
) -> Result<Vec<u8>, BotError> {
    let stickers = stickers
        .into_iter()
        .filter_map(|s| s.thumbnail_file_id)
        .collect_vec();
    let grid_size = get_grid_size(stickers.len())?;
    let mut img = <ImageBuffer<Rgba<u8>, _>>::new(size, size);
    let futures: FuturesUnordered<_> = stickers
        .iter()
        .take(grid_size * grid_size)
        .map(|thumbnail_file_id| {
            Box::pin(fetch_sticker_file( thumbnail_file_id.clone(), bot.clone()))
        })
        .collect();
    let thumb_size = size / grid_size as u32;

    for (i, future) in futures.into_iter().enumerate() {
        let (value, _) = future.await?;
        let dynamic_image = image::load_from_memory(&value)?;
        let dynamic_image = dynamic_image.resize(thumb_size, thumb_size, image::imageops::FilterType::Triangle);
        let square_top_left = ((i / grid_size) as u32 * thumb_size, (i % grid_size) as u32 * thumb_size);
        let x = square_top_left.0 + ((thumb_size - dynamic_image.width()) / 2); // center if thumbnail is not square
        let y = square_top_left.1 + ((thumb_size - dynamic_image.height()) / 2); // center if thumbnail is not square
        img.copy_from(&dynamic_image, x, y);
    }

    let mut bytes: Vec<u8> = Vec::new();
    img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)?;
    Ok(bytes)
}

fn get_grid_size(sticker_count: usize) -> anyhow::Result<usize> {
    match sticker_count {
        0 => Err(anyhow::anyhow!("no stickers")),
        1 => Ok(1),
        2..=4 => Ok(2),
        5.. => Ok(3),
    }
}
