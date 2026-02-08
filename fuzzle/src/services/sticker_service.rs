use chrono::NaiveDateTime;
use itertools::Itertools;

use crate::{bot::InternalError, database::{Database, Sticker, StickerSet}, util::format_relative_time};


#[derive(Clone)]
pub struct StickerService {
    database: Database,
}

impl StickerService {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub async fn get_sticker_set_timeline(
        &self,
        set_id: &str,
    ) -> Result<Vec<(String, Vec<Sticker>)>, InternalError> {
        let stickers = self.database.get_all_stickers_in_set(set_id).await?;

    let groups = stickers
        .into_iter()
        .sorted_by_key(|s| s.created_at) // TODO: should we use the sticker_file created_at here?
        .rev()
        .chunk_by(|s| format_relative_time(s.created_at))
        .into_iter()
        .map(|(relative_time, stickers)| (format!("{}", relative_time), stickers.collect_vec()))
        .collect_vec();

        Ok(groups)
    }

    pub async fn get_all_sticker_timeline(
        &self,
        limit: i64,
        before: NaiveDateTime,
    ) -> Result<Option<(NaiveDateTime, Vec<(String, Vec<Sticker>)>)>, InternalError> {
        let stickers = self.database.get_latest_stickers(limit, before).await?;
        let Some(after) = stickers.last().map(|sticker| sticker.created_at) else {
            return Ok(None);
        };

        let groups = stickers
            .into_iter()
            .sorted_by_key(|s| s.created_at) // TODO: should we use the sticker_file created_at here?
            .rev()
            .chunk_by(|s| format!("{}", s.created_at.format("%B %-d, %Y ~%l %P")))
            .into_iter()
            .map(|(relative_time, stickers)| (format!("{}", relative_time), stickers.collect_vec()))
            .collect_vec();

        Ok(Some((after, groups)))
    }

    pub async fn get_all_sticker_set_timeline(
        &self,
        limit: i64,
        before: NaiveDateTime,
    ) -> Result<Option<(NaiveDateTime, Vec<(String, Vec<StickerSet>)>)>, InternalError> {
        let sets = self.database.get_latest_sticker_sets(limit, before).await?;
        let Some(after) = sets.last().map(|set| set.created_at) else {
            return Ok(None);
        };

        let groups = sets
            .into_iter()
            .sorted_by_key(|s| s.created_at) // TODO: should we use the sticker_file created_at here?
            .rev()
            .chunk_by(|s| format!("{}", s.created_at.format("%B %-d, %Y")))
            .into_iter()
            .map(|(relative_time, sets)| (format!("{}", relative_time), sets.collect_vec()))
            .collect_vec();

        Ok(Some((after, groups)))
    }
}
