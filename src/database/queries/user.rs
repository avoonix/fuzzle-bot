use itertools::Itertools;

use crate::database::UserSettings;
use crate::database::{model::User, raw::RawDatabaseUser};

use super::DatabaseError;

use super::Database;

impl Database {
    // pub async fn export_user_data(&self) -> Result<Vec<User>, DatabaseError> {
    //     let users = sqlx::query_as!(RawDatabaseUser, "SELECT * FROM user")
    //         .fetch_all(&self.pool)
    //         .await?;

    //     users
    //         .into_iter()
    //         .map(|user| user.try_into().map_err(DatabaseError::from))
    //         .collect()
    // }

    pub async fn update_settings(
        &self,
        user_id: u64,
        settings: UserSettings,
    ) -> Result<(), DatabaseError> {
        let user_id = user_id as i64; // TODO: no convert
        let settings = serde_json::to_string(&settings)?;
        let rows_affected = sqlx::query!(
            "UPDATE user SET settings = ?1 WHERE id = ?2",
            settings,
            user_id
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

    pub async fn add_tag_to_blacklist(
        &self,
        user_id: u64,
        tag: String,
    ) -> Result<(), DatabaseError> {
        let user_id = user_id as i64; // TODO: no convert
        let rows_affected = sqlx::query!(
            "UPDATE user SET blacklist = json_insert(blacklist, '$[#]', ?1) WHERE id = ?2",
            tag,
            user_id
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

    pub async fn update_user(
        &self,
        user_id: u64,
        blacklist: Vec<String>,
    ) -> Result<(), DatabaseError> {
        let user_id = user_id as i64; // TODO: no convert
        let blacklist = serde_json::to_string(&blacklist)?;
        sqlx::query!(
            "UPDATE user SET blacklist = ?1 WHERE id = ?2",
            blacklist,
            user_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_blacklisted_tag(
        &self,
        user_id: u64,
        tag: String,
    ) -> Result<(), DatabaseError> {
        let current_blacklist = self
            .get_user(user_id)
            .await?
            .ok_or(anyhow::anyhow!("user does not exist (should never happen)"))?
            .blacklist;
        let new_blacklist = current_blacklist
            .into_iter()
            .filter(|blacklisted_tag| blacklisted_tag != &tag)
            .collect_vec();
        self.update_user(user_id, new_blacklist).await
    }

    pub async fn get_user(&self, user_id: u64) -> Result<Option<User>, DatabaseError> {
        let user_id = user_id as i64; // TODO: no convert
        let user = sqlx::query_as!(RawDatabaseUser, "SELECT * FROM user WHERE id = ?1", user_id)
            .fetch_optional(&self.pool)
            .await?;
        match user {
            Some(user) => Ok(Some(user.try_into()?)),
            None => Ok(None),
        }
    }

    pub async fn create_user(
        &self,
        user_id: u64,
        default_blacklist: Vec<String>,
    ) -> Result<User, DatabaseError> {
        let user_id = user_id as i64; // TODO: no convert
        let default_blacklist = serde_json::to_string(&default_blacklist)?;
        let user = sqlx::query_as!(
            RawDatabaseUser,
            "INSERT INTO user (id, blacklist) VALUES (?1, ?2) RETURNING *",
            user_id,
            default_blacklist
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(user.try_into()?)
    }

    pub async fn add_recently_used_sticker(
        &self,
        user_id: u64,
        sticker_unique_id: String,
    ) -> Result<(), DatabaseError> {
        let user_id = user_id as i64; // TODO: no convert
        sqlx::query!("INSERT INTO sticker_user (sticker_id, user_id) VALUES (?1, ?2)
                                        ON CONFLICT(sticker_id, user_id) DO UPDATE SET last_used = datetime('now')", sticker_unique_id, user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn clear_recently_used_stickers(&self, user_id: u64) -> Result<(), DatabaseError> {
        let user_id = user_id as i64; // TODO: no convert
        sqlx::query!("DELETE FROM sticker_user WHERE user_id = ?1", user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
