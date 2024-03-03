use base64::{engine::general_purpose, Engine};
use itertools::Itertools;
use sqlx::Row;

use crate::{
    database::{
        model::{Relationship, SavedSticker, SavedStickerSet}, query_builder::StickerTagQuery, FileAnalysis, FileAnalysisWithStickerId, FileInfo, Order
    },
    util::Emoji,
};

use super::DatabaseError;

use super::Database;

impl Database {
    pub async fn update_last_fetched(&self, set_name: String) -> Result<(), DatabaseError> {
        let rows_affected = sqlx::query!(
            "UPDATE sticker_set SET last_fetched = datetime('now') WHERE id = ?1",
            set_name
        )
        .execute(&self.pool)
        .await?
        .rows_affected();

        if rows_affected == 0 {
            Err(DatabaseError::NoRowsAffected)
        } else {
            Ok(())
        }
    }

    pub async fn sticker_set_fetched_within_duration(
        &self,
        set_name: String,
        duration: chrono::Duration,
    ) -> Result<bool, DatabaseError> {
        let record = sqlx::query!(
            "SELECT last_fetched FROM sticker_set WHERE id = ?1",
            set_name
        )
        .fetch_optional(&self.pool)
        .await?;

        record.map_or_else(
            || Ok(false),
            |record| {
                record.last_fetched.map_or_else(
                    || Ok(false),
                    |last_fetched| {
                        let now = chrono::Utc::now().naive_utc();
                        let time_since_last_fetch = now - last_fetched;
                        Ok(time_since_last_fetch < duration)
                    },
                )
            },
        )
    }

    pub async fn get_sticker_set(
        &self,
        set_name: String,
    ) -> Result<Option<SavedStickerSet>, DatabaseError> {
        let set: Option<SavedStickerSet> = sqlx::query_as!(
            SavedStickerSet,
            "SELECT * FROM sticker_set WHERE id = ?1",
            set_name
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(set)
    }

    pub async fn update_sticker(
        &self,
        sticker_unique_id: String,
        file_id: String,
    ) -> Result<(), DatabaseError> {
        // i dont think the other attributes can change without resulting in a different sticker_unique_id
        sqlx::query!(
            "UPDATE sticker SET file_id = ?1 WHERE id = ?2",
            file_id,
            sticker_unique_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// title is sometimes not known immediately
    /// does not update last_fetched
    pub async fn create_sticker_set(
        &self,
        name: String,
        title: Option<String>,
        is_animated: bool,
    ) -> Result<(), DatabaseError> {
        if let Some(title) = title {
            sqlx::query!(
                "INSERT INTO sticker_set (id, title, is_animated, last_fetched) VALUES (?1, ?2, ?3, NULL)
                     ON CONFLICT(id) DO UPDATE SET title = ?2, is_animated = ?3",
                name,
                title,
                is_animated
            )
            // "INSERT INTO sticker_set (id, title, last_fetched) VALUES (?1, ?2, datetime('now'))
            //          ON CONFLICT(id) DO UPDATE SET title = ?2, last_fetched = datetime('now')
            //          ",
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query!(
                "INSERT INTO sticker_set (id, title, is_animated, last_fetched) VALUES (?1, NULL, ?2, NULL)
                     ON CONFLICT(id) DO NOTHING",
                name,
                is_animated
            )
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub async fn create_file(&self, file_hash: String) -> Result<(), DatabaseError> {
        sqlx::query!(
            "INSERT INTO file_hash (id) VALUES (?1) ON CONFLICT(id) DO NOTHING",
            file_hash
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_sticker(
        &self,
        sticker_unique_id: String,
        sticker_file_id: String,
        emojis: Vec<Emoji>,
        sticker_set: String,
        file_hash: String,
    ) -> Result<(), DatabaseError> {
        let emoji = emojis.iter().map(std::string::ToString::to_string).join("");
        sqlx::query!("INSERT INTO sticker (id, set_id, file_id, file_hash, emoji) VALUES (?1               , ?2         , ?3             , ?4       , ?5   )",
                                                                                          sticker_unique_id, sticker_set, sticker_file_id, file_hash, emoji)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_set(
        &self,
        sticker_unique_id: String,
    ) -> Result<Option<SavedStickerSet>, DatabaseError> {
        let set: Option<SavedStickerSet> = sqlx::query_as!(
            SavedStickerSet,
            "SELECT * FROM sticker_set WHERE id = (SELECT set_id FROM sticker WHERE id = ?1)",
            sticker_unique_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(set)
    }

    pub async fn get_all_stickers_in_set(
        &self,
        set_name: String,
    ) -> Result<Vec<SavedSticker>, DatabaseError> {
        let stickers = sqlx::query!("SELECT * FROM sticker WHERE set_id = ?1", set_name)
            .fetch_all(&self.pool)
            .await?;

        Ok(stickers
            .into_iter()
            .map(|sticker| SavedSticker {
                file_id: sticker.file_id,
                id: sticker.id,
                file_hash: sticker.file_hash,
                emoji: None,
                set_id: sticker.set_id,
            })
            .collect_vec())
    }

    pub async fn get_sticker(
        &self,
        sticker_unique_id: String,
    ) -> Result<Option<SavedSticker>, DatabaseError> {
        let sticker = sqlx::query!("SELECT * FROM sticker WHERE id = ?1", sticker_unique_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(sticker.map(|sticker| SavedSticker {
            file_id: sticker.file_id,
            id: sticker.id,
            file_hash: sticker.file_hash,
            emoji: Emoji::parse(&sticker.emoji).first().cloned(),
            set_id: sticker.set_id,
        }))
    }

    pub async fn get_sticker_tags(
        &self,
        sticker_unique_id: String,
    ) -> Result<Vec<String>, DatabaseError> {
        let tags: Vec<String> = sqlx::query_scalar!("SELECT tag FROM file_hash_tag WHERE file_hash = (SELECT file_hash FROM sticker WHERE id = ?1)", sticker_unique_id)
            .fetch_all(&self.pool)
            .await?;

        Ok(tags)
    }

    pub async fn get_stickers_by_emoji(
        &self,
        emoji: Emoji,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SavedSticker>, DatabaseError> {
        let limit = limit as i64;
        let offset = offset as i64;
        let emoji = emoji.to_string();
        let stickers = sqlx::query!(
            "SELECT * FROM sticker WHERE emoji = ?1 GROUP BY file_hash ORDER BY rowid DESC LIMIT ?2 OFFSET ?3",
            emoji,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(stickers
            .into_iter()
            .map(|sticker| SavedSticker {
                file_id: sticker.file_id,
                id: sticker.id,
                file_hash: sticker.file_hash,
                emoji: None,
                set_id: sticker.set_id,
            })
            .collect_vec())
    }

    pub async fn get_stickers_for_tag_query(
        &self,
        tags: Vec<String>,
        blacklist: Vec<String>,
        limit: usize,
        offset: usize,
        order: Order,
    ) -> Result<Vec<SavedSticker>, DatabaseError> {
        let tags = tags.into_iter().collect_vec();
        let blacklist = blacklist.into_iter().collect_vec();
        let query = StickerTagQuery::new(tags, blacklist)
            .limit(limit)
            .offset(offset)
            .order(order);

        let stickers = query.generate().build().fetch_all(&self.pool).await?;

        Ok(stickers
            .into_iter()
            .map(|row| SavedSticker {
                file_id: row.get("file_id"),
                id: row.get("id"),
                file_hash: row.get("file_hash"),
                emoji: None,
                set_id: row.get("set_id"),
            })
            .collect_vec())
    }

    pub async fn get_random_sticker_to_tag(&self) -> Result<Option<SavedSticker>, DatabaseError> {
        let sticker = sqlx::query!("SELECT * FROM sticker WHERE file_hash NOT IN (SELECT file_hash FROM file_hash_tag) ORDER BY RANDOM() LIMIT 1")
            .fetch_optional(&self.pool)
            .await?;

        Ok(sticker.map(|sticker| SavedSticker {
            file_id: sticker.file_id,
            id: sticker.id,
            file_hash: sticker.file_hash,
            emoji: None,
            set_id: sticker.set_id,
        }))
    }

    pub async fn export_file_looks_like_visual_hash_relationships(
        &self,
    ) -> Result<Vec<Relationship>, DatabaseError> {
        let relationships = sqlx::query!("SELECT id AS file_hash, visual_hash FROM file_analysis")
            .fetch_all(&self.pool)
            .await?;

        Ok(relationships
            .into_iter()
            .filter_map(|relationship| {
                relationship.visual_hash.map(|visual_hash| Relationship {
                    in_: relationship.file_hash,
                    out: general_purpose::URL_SAFE_NO_PAD.encode(visual_hash),
                })
            })
            .collect_vec())
    }

    pub async fn export_file_tagged_tag_relationships(
        &self,
    ) -> Result<Vec<Relationship>, DatabaseError> {
        let relationships = sqlx::query!("SELECT file_hash, tag FROM file_hash_tag")
            .fetch_all(&self.pool)
            .await?;

        Ok(relationships
            .into_iter()
            .map(|relationship| Relationship {
                in_: relationship.file_hash,
                out: relationship.tag,
            })
            .collect_vec())
    }

    pub async fn export_sticker_is_a_file_relationship(
        &self,
    ) -> Result<Vec<Relationship>, DatabaseError> {
        let relationships = sqlx::query!("SELECT id, file_hash FROM sticker")
            .fetch_all(&self.pool)
            .await?;

        Ok(relationships
            .into_iter()
            .map(|relationship| Relationship {
                in_: relationship.id,
                out: relationship.file_hash,
            })
            .collect_vec())
    }

    pub async fn export_set_contains_sticker_relationship(
        &self,
    ) -> Result<Vec<Relationship>, DatabaseError> {
        let relationships = sqlx::query!("SELECT set_id, id FROM sticker")
            .fetch_all(&self.pool)
            .await?;

        Ok(relationships
            .into_iter()
            .map(|relationship| Relationship {
                in_: relationship.set_id,
                out: relationship.id,
            })
            .collect_vec())
    }

    pub async fn get_file_info(
        &self,
        sticker_unique_id: String,
    ) -> Result<Option<FileInfo>, DatabaseError> {
        let file_info: Option<FileInfo> = sqlx::query_as!(
            FileInfo,
            "select count(distinct sticker.id) as sticker_count, file_hash.created_at, file_hash.id from file_hash join sticker on sticker.file_hash = file_hash.id where file_hash.id = (select file_hash from sticker where id = ?1);",
            sticker_unique_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(file_info)
    }

    pub async fn get_sets_containing_sticker(
        &self,
        sticker_unique_id: String,
    ) -> Result<Vec<SavedStickerSet>, DatabaseError> {
        let sets: Vec<SavedStickerSet> = sqlx::query_as!(
            SavedStickerSet,
            "SELECT * FROM sticker_set WHERE id IN (SELECT set_id FROM sticker WHERE file_hash IN (SELECT file_hash FROM sticker WHERE id = ?1))",
            sticker_unique_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(sets)
    }

    pub async fn set_locked(
        &self,
        sticker_unique_id: String,
        user_id: u64,
        locked: bool,
    ) -> Result<(), DatabaseError> {
        let user_id = user_id as i64;
        if locked {
            sqlx::query!(
            "UPDATE file_hash SET tags_locked_by_user_id = ?1 WHERE id = (SELECT file_hash FROM sticker WHERE id = ?2)",
            user_id,
            sticker_unique_id
        )
        .fetch_optional(&self.pool)
        .await?;
        } else {
            sqlx::query!(
            "UPDATE file_hash SET tags_locked_by_user_id = NULL WHERE id = (SELECT file_hash FROM sticker WHERE id = ?1)",
            sticker_unique_id
        )
        .fetch_optional(&self.pool)
        .await?;
        }

        Ok(())
    }

    pub async fn sticker_is_locked(
        &self,
        sticker_unique_id: String,
    ) -> Result<bool, DatabaseError> {
        let locked_by_user: Option<Option<i64>> = sqlx::query_scalar!(
            "SELECT tags_locked_by_user_id FROM file_hash WHERE id = (SELECT file_hash FROM sticker WHERE id = ?1)",
            sticker_unique_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(locked_by_user.flatten().is_some())
    }

    pub async fn get_set_name(
        &self,
        sticker_unique_id: String,
    ) -> Result<Option<String>, DatabaseError> {
        let set_name: Option<String> = sqlx::query_scalar!(
            "SELECT set_id FROM sticker WHERE id = ?1",
            sticker_unique_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(set_name)
    }

    pub async fn update_thumbnail(
        &self,
        file_hash: String,
        thumbnail_file_id: String,
    ) -> Result<(), DatabaseError> {
        sqlx::query!(
            "INSERT INTO file_analysis (id, thumbnail_file_id) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET thumbnail_file_id = ?2",
            file_hash,
            thumbnail_file_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_visual_hash(
        &self,
        file_hash: String,
        visual_hash: Vec<u8>,
    ) -> Result<(), DatabaseError> {
        sqlx::query!(
            "INSERT INTO file_analysis (id, visual_hash) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET visual_hash = ?2",
            file_hash,
            visual_hash
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_histogram(
        &self,
        file_hash: String,
        histogram: Vec<u8>,
    ) -> Result<(), DatabaseError> {
        sqlx::query!(
            "INSERT INTO file_analysis (id, histogram) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET histogram = ?2",
            file_hash,
            histogram
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_embedding(
        &self,
        file_hash: String,
        embedding: Vec<u8>,
    ) -> Result<(), DatabaseError> {
        sqlx::query!( "INSERT INTO file_analysis (id, embedding) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET embedding = ?2", file_hash, embedding)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_n_stickers_with_missing_analysis(
        &self,
        n: i64,
    ) -> Result<Vec<FileAnalysis>, DatabaseError> {
        let analysis: Vec<FileAnalysis> = sqlx::query_as!(
            FileAnalysis,
            "SELECT * FROM file_analysis WHERE visual_hash IS NULL OR histogram IS NULL OR embedding IS NULL ORDER BY random() LIMIT ?1",
            n
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(analysis)
    }

    pub async fn get_analysis_for_sticker_id(
        &self,
        sticker_id: String,
    ) -> Result<Option<FileAnalysis>, DatabaseError> {
        let analysis: Option<FileAnalysis> = sqlx::query_as!(
            FileAnalysis,
            "SELECT * FROM file_analysis WHERE id = (SELECT file_hash FROM sticker WHERE id = ?1)",
            sticker_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(analysis)
    }

    pub async fn get_analysis_for_all_stickers_with_tags(
        &self,
    ) -> Result<Vec<FileAnalysisWithStickerId>, DatabaseError> {
        let analysis: Vec<FileAnalysisWithStickerId> =
            sqlx::query_as!(FileAnalysisWithStickerId, "SELECT file_analysis.*, sticker.id AS sticker_id FROM file_analysis INNER JOIN sticker WHERE sticker.file_hash = file_analysis.id AND EXISTS (SELECT * FROM file_hash_tag WHERE file_hash_tag.file_hash = sticker.file_hash) GROUP BY file_analysis.id")
                .fetch_all(&self.pool)
                .await?;
        Ok(analysis)
    }

    pub async fn get_analysis_for_all_stickers(
        &self,
    ) -> Result<Vec<FileAnalysisWithStickerId>, DatabaseError> {
        let analysis: Vec<FileAnalysisWithStickerId> =
            sqlx::query_as!(FileAnalysisWithStickerId, "SELECT file_analysis.*, sticker.id AS sticker_id FROM file_analysis INNER JOIN sticker WHERE sticker.file_hash = file_analysis.id GROUP BY file_analysis.id")
                .fetch_all(&self.pool)
                .await?;
        Ok(analysis)
    }

    pub async fn unban_set(&self, set_name: String) -> Result<(), DatabaseError> {
        sqlx::query!("DELETE FROM removed_set WHERE id = ?1", set_name)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn ban_set(&self, set_name: String) -> Result<(), DatabaseError> {
        self.delete_sticker_set(set_name.clone()).await?;
        sqlx::query!(
            "INSERT INTO removed_set (id) VALUES (?1) ON CONFLICT(id) DO NOTHING",
            set_name
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_sticker_set(&self, set_name: String) -> Result<(), DatabaseError> {
        sqlx::query!("DELETE FROM sticker_set WHERE id = ?1", set_name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_sticker(&self, sticker_unique_id: String) -> Result<(), DatabaseError> {
        sqlx::query!("DELETE FROM sticker WHERE id = ?1", sticker_unique_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_n_least_recently_fetched_set_ids(
        &self,
        n: i64,
    ) -> Result<Vec<String>, DatabaseError> {
        let sets: Vec<SavedStickerSet> = sqlx::query_as!(
            SavedStickerSet,
            "SELECT * FROM sticker_set order by last_fetched limit ?1",
            n
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(sets.into_iter().map(|s| s.id).collect_vec())
    }

    pub async fn get_recently_used_stickers(
        &self,
        user_id: u64,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SavedSticker>, DatabaseError> {
        let user_id = user_id as i64; // TODO: no convert
        let limit = limit as i64;
        let offset = offset as i64;
        // TODO: sort by recently used, sort by favorites
        let stickers = sqlx::query!("SELECT sticker.* FROM sticker INNER JOIN sticker_user ON sticker_user.sticker_id = sticker.id WHERE sticker_user.user_id = ?1 ORDER BY last_used DESC LIMIT ?2 OFFSET ?3", user_id, limit, offset)
            .fetch_all(&self.pool)
            .await?;

        Ok(stickers
            .into_iter()
            .map(|sticker| SavedSticker {
                file_id: sticker.file_id,
                id: sticker.id,
                file_hash: sticker.file_hash,
                emoji: None,
                set_id: sticker.set_id,
            })
            .collect_vec())
    }
}
