use diesel::dsl::now;
use diesel::{delete, insert_into, prelude::*, update};

use crate::database::{UserSettings, UserStats, Blacklist, User, DialogState};

use super::DatabaseError;

use super::Database;

use super::super::schema::*;

impl Database {
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn update_dialog_state(
        &self,
        user_id: i64,
        dialog_state: &DialogState,
    ) -> Result<(), DatabaseError> {
        let updated_rows = update(user::table.find(user_id))
            .set(user::dialog_state.eq(Some(dialog_state)))
            .execute(&mut self.pool.get()?)?;
        #[cfg(debug_assertions)]
        assert_eq!(updated_rows, 1);
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn update_settings(
        &self,
        user_id: i64,
        user_settings: &UserSettings,
    ) -> Result<(), DatabaseError> {
        let updated_rows = update(user::table.find(user_id))
            .set(user::settings.eq(Some(user_settings)))
            .execute(&mut self.pool.get()?)?;
        #[cfg(debug_assertions)]
        assert_eq!(updated_rows, 1);
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn update_user_blacklist(
        &self,
        user_id: i64,
        new_blacklist: Blacklist,
    ) -> Result<(), DatabaseError> {
        let updated_rows = diesel::update(user::table.find(user_id))
            .set(user::blacklist.eq(new_blacklist))
            .execute(&mut self.pool.get()?)?;
        #[cfg(debug_assertions)]
        assert_eq!(updated_rows, 1);
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_user_by_id(&self, user_id: i64) -> Result<Option<User>, DatabaseError> {
        Ok(user::table
            .filter(user::id.eq(user_id))
            .select(User::as_select())
            .first(&mut self.pool.get()?)
            .optional()?)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn create_user(
        &self,
        user_id: i64,
        default_blacklist: Blacklist,
    ) -> Result<User, DatabaseError> {
        Ok(insert_into(user::table)
            .values((user::id.eq(user_id), user::blacklist.eq(default_blacklist)))
            .get_result(&mut self.pool.get()?)?)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn set_recently_used_sticker_favorite(
        &self,
        user_id: i64,
        sticker_id: &str,
        is_favorite: bool,
    ) -> Result<(), DatabaseError> {
        insert_into(sticker_user::table)
            .values((
                sticker_user::sticker_id.eq(sticker_id),
                sticker_user::user_id.eq(user_id),
                sticker_user::is_favorite.eq(is_favorite),
            ))
            .on_conflict((sticker_user::sticker_id, sticker_user::user_id))
            .do_update()
            .set((
                sticker_user::last_used.eq(now),
                sticker_user::is_favorite.eq(is_favorite),
            ))
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn add_recently_used_sticker(
        &self,
        user_id: i64,
        sticker_id: &str,
    ) -> Result<(), DatabaseError> {
        insert_into(sticker_user::table)
            .values((
                sticker_user::sticker_id.eq(sticker_id),
                sticker_user::user_id.eq(user_id),
            ))
            .on_conflict((sticker_user::sticker_id, sticker_user::user_id))
            .do_update()
            .set(sticker_user::last_used.eq(now))
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn clear_recently_used_stickers(&self, user_id: i64) -> Result<(), DatabaseError> {
        delete(
            sticker_user::table
                .filter(sticker_user::user_id.eq(user_id))
                .filter(sticker_user::is_favorite.eq(false)),
        )
        .execute(&mut self.pool.get()?)?;
        Ok(())
    }
}
