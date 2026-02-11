use chrono::NaiveDateTime;
use itertools::Itertools;

use crate::{
    bot::{BotError, InternalError, UserError},
    database::{Database, Sticker, StickerSet},
    inline::SimilarityAspect,
    qdrant::VectorDatabase,
    services::ImportService,
    sticker::{Match, resolve_file_hashes_to_sticker_ids_and_clean_up_unreferenced_files},
    util::{Required, format_relative_time},
};

#[derive(Clone)]
pub struct SimilarityService {
    database: Database,
    vector_db: VectorDatabase,
    import: ImportService,
}

impl SimilarityService {
    pub fn new(database: Database, vector_db: VectorDatabase, import: ImportService) -> Self {
        Self {
            database,
            vector_db,
            import,
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn find_similar_stickers(
        &self,
        sticker_id: String,
        aspect: SimilarityAspect,
        limit: u64,
        offset: u64,
    ) -> Result<(Vec<Match>, usize), BotError> {
        // TODO: use internal error
        let sticker = self
            .database
            .get_sticker_by_id(&sticker_id)
            .await?
            .required()?;
        let score_threshold = 0.0;

        // let result = vector_db.find_similar_stickers(query_embedding.clone().into()).await?;
        let file_hashes = self
            .vector_db
            .find_similar_stickers(
                &[sticker.sticker_file_id.clone()],
                &[],
                aspect,
                score_threshold,
                limit,
                offset,
            )
            .await?;
        let file_hashes = match file_hashes {
            Some(hashes) => hashes,
            None => {
                // dispatch in background - otherwise the query would take too long if the set is large
                if !self.import.is_busy() {
                    self.import
                        .queue_sticker_set_import(&sticker.sticker_set_id, false, None, None)
                        .await;
                }
                return Err(UserError::VectorNotFound.into());
            }
        };

        let len = file_hashes.len();
        Ok((
            resolve_file_hashes_to_sticker_ids_and_clean_up_unreferenced_files(
                self.database.clone(),
                self.vector_db.clone(),
                file_hashes,
            )
            .await?,
            len,
        ))
    }

    pub async fn matches_to_stickers(&self, matches: Vec<Match>) -> Result<Vec<(Sticker, f32)>, InternalError> {
        let mut stickers = Vec::new();
        for m in matches {
            if let Some(sticker) = self.database.get_sticker_by_id(&m.sticker_id).await? {
                stickers.push((sticker, m.distance));
            } else {
                tracing::warn!(%m.sticker_id, "could not find sticker");
            }
        }
        Ok(stickers)
    }
}
