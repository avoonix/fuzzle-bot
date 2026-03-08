use anyhow::Result;
use base64::{engine::general_purpose, Engine};
use blake2::{digest::consts::U16, Blake2b, Digest};

use crate::util::StickerFileId;

type Blake2b128 = Blake2b<U16>;
pub fn calculate_sticker_file_hash(buf: Vec<u8>) -> Result<StickerFileId> {
    let hash = Blake2b128::digest(buf);
    Ok(general_purpose::URL_SAFE_NO_PAD.encode(hash).into())
}
