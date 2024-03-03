use serde::{Deserialize, Serialize};

use super::StickerSetDto;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerMergeInfos {
    pub can_be_merged: bool,
    pub embedding_similarity: f32,
    pub histogram_similarity: f32,
    pub visual_hash_similarity: f32,
    pub sticker_a: StickerMergeInfo,
    pub sticker_b: StickerMergeInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerMergeInfo {
    pub days_known: i64,
    pub instance_count: i64,
    pub sticker_sets: Vec<StickerSetDto>,
}
