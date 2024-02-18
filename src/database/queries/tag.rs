use itertools::Itertools;
use std::collections::HashMap;

use crate::database::model::PopularTag;
use crate::database::UserStats;

use super::DatabaseError;

use super::Database;

impl Database {
    pub async fn tag_sticker(
        &self,
        sticker_unique_id: String,
        tag_names: Vec<String>,
        user: Option<u64>,
    ) -> Result<(), DatabaseError> {
        let user = user.map(|u| u as i64);
        for tag_name in tag_names {
            sqlx::query!("INSERT INTO file_hash_tag (file_hash, tag, added_by_user_id) VALUES ((SELECT file_hash FROM sticker WHERE id = ?1), ?2, ?3)
                         ON CONFLICT(file_hash, tag) DO NOTHING ", sticker_unique_id, tag_name, user)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// except locked stickers
    pub async fn tag_all_stickers_in_set(
        &self,
        set_name: String,
        tags: Vec<String>,
        user: u64,
    ) -> Result<u64, DatabaseError> {
        let user = user as i64;
        let mut tags_affected = 0;
        for tag in tags {
            tags_affected += sqlx::query!("INSERT INTO file_hash_tag (file_hash, tag, added_by_user_id)
                                           SELECT DISTINCT file_hash, ?1, ?2 FROM sticker 
                                                WHERE set_id = ?3 AND NOT EXISTS (SELECT * FROM file_hash WHERE sticker.file_hash = file_hash.id AND file_hash.tags_locked_by_user_id IS NOT NULL)
                                           ON CONFLICT (file_hash, tag) DO NOTHING", tag, user, set_name)
                .execute(&self.pool)
                .await?
                .rows_affected();
        }

        Ok(tags_affected)
    }

    /// except locked stickers
    pub async fn untag_all_stickers_in_set(
        &self,
        set_name: String,
        tag: String,
        user: u64,
    ) -> Result<u64, DatabaseError> {
        let user = user as i64;

        let tags_affected = sqlx::query!("INSERT INTO file_hash_tag_history (file_hash, tag, removed_by_user_id, added_by_user_id)
                      SELECT file_hash, tag, ?1, added_by_user_id FROM file_hash_tag WHERE tag = ?2 AND file_hash IN (SELECT file_hash FROM sticker 
                            WHERE set_id = ?3 AND NOT EXISTS (SELECT * FROM file_hash WHERE sticker.file_hash = file_hash.id AND file_hash.tags_locked_by_user_id IS NOT NULL))",
                      user, tag, set_name
                    )
                .execute(&self.pool)
                .await?
                .rows_affected();

        Ok(tags_affected)
    }

    pub async fn get_popular_tags(&self, limit: u64) -> Result<Vec<PopularTag>, DatabaseError> {
        let limit = limit as i64; // TODO: no convert
        let tags = sqlx::query!("SELECT tag, COUNT(*) AS count FROM file_hash_tag GROUP BY tag ORDER BY count DESC LIMIT ?1", limit)
            .fetch_all(&self.pool)
            .await?;

        Ok(tags
            .into_iter()
            .map(|tag| PopularTag {
                name: tag.tag,
                count: tag.count as u64,
            })
            .collect_vec())
    }

    pub async fn untag_sticker(
        &self,
        sticker_unique_id: String,
        tag_name: String,
        user: u64,
    ) -> Result<(), DatabaseError> {
        let user = user as i64;
        // will error if the user tries to remove a tag that does not exist
        let rows_affected = sqlx::query!("INSERT INTO file_hash_tag_history (file_hash, tag, removed_by_user_id, added_by_user_id)
                      SELECT file_hash, tag, ?1, added_by_user_id FROM file_hash_tag WHERE tag = ?2 AND file_hash = (SELECT file_hash FROM sticker WHERE id = ?3)",
                      user, tag_name, sticker_unique_id
                    )
            .execute(&self.pool)
            .await?
            .rows_affected();
        // we only need to insert the history entry; a trigger will delete the tag relationship

        // TODO: return a norowsaffected error and make the caller handle it -> in continuous tag mode, you can then inform the user that
        // the tag already existed

        // if rows_affected != 1 {
        //     dbg!(rows_affected);
        //     return Err(DatabaseError::NoRowsAffected);
        // }
        Ok(())
    }

    pub async fn suggest_tags_for_sticker_based_on_other_stickers_in_set(
        &self,
        sticker_unique_id: String,
    ) -> Result<Vec<PopularTag>, DatabaseError> {
        // TODO: redo query
        let tags = sqlx::query!(
            "SELECT tag, count(*) as \"count!: i64\" FROM file_hash_tag WHERE file_hash IN
                                (SELECT file_hash FROM sticker WHERE set_id =
                                    (SELECT set_id FROM sticker WHERE id = ?1))
                                GROUP BY tag ORDER BY \"count!: i64\" DESC",
            sticker_unique_id
        )
        // let tags = sqlx::query!(
        //     "SELECT tag, COUNT(*) as count FROM file_hash_tag GROUP BY tag ORDER BY count DESC"
        // )
        .fetch_all(&self.pool)
        .await?;

        Ok(tags
            .into_iter()
            .map(|tag| PopularTag {
                name: tag.tag,
                count: tag.count as u64, // TODO: no convert
            })
            .collect_vec())
    }

    pub async fn get_user_tagging_stats_24_hours(
        &self,
    ) -> Result<HashMap<Option<i64>, UserStats>, DatabaseError> {
        let added_result = sqlx::query!("select added_by_user_id as user_id, count(*) as \"count: i64\" from file_hash_tag where julianday('now') - julianday(created_at) <= 1 group by added_by_user_id")
            .fetch_all(&self.pool)
            .await?;
        let removed_result = sqlx::query!("select removed_by_user_id as user_id, count(*) as \"count: i64\" from file_hash_tag_history where julianday('now') - julianday(created_at) <= 1 group by removed_by_user_id")
            .fetch_all(&self.pool)
            .await?;

        let mut result: HashMap<Option<i64>, UserStats> = HashMap::new();
        for added in added_result {
            result.entry(added.user_id).or_default().added_tags = added.count;
        }
        for removed in removed_result {
            result.entry(removed.user_id).or_default().removed_tags = removed.count;
        }

        Ok(result)
    }
}
