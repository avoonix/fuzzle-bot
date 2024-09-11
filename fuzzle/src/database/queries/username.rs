use diesel::delete;
use diesel::dsl::count_star;
use diesel::dsl::exists;
use diesel::dsl::not;
use diesel::dsl::now;
use diesel::dsl::sql;
use diesel::insert_into;
use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use diesel::sql_query;
use diesel::sql_types::BigInt;
use diesel::sql_types::Text;
use itertools::Itertools;
use num_traits::ToPrimitive;
use r2d2::PooledConnection;
use std::collections::HashMap;
use teloxide::types::ChatId;
use teloxide::types::UserId;

use crate::database::model::PopularTag;
use crate::database::Tag;
use crate::database::UserStats;
use crate::database::UsernameKind;
use crate::tags::Category;
use crate::util::Emoji;

use super::sticker::max;
use super::DatabaseError;

use super::Database;

use super::super::schema::*;

impl Database {
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn add_username(&self, name: &str) -> Result<(), DatabaseError> {
        insert_into(username::table)
        .values((
            username::tg_username.eq(name),
        ))
        .on_conflict_do_nothing()
        .execute(&mut self.pool.get()?)?;

        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn add_username_details(&self, name: &str, kind: UsernameKind, telegram_id: i64) -> Result<(), DatabaseError> {
        insert_into(username::table)
        .values((
            username::tg_username.eq(name),
            username::kind.eq(kind),
            username::tg_id.eq(Some(telegram_id)),
        ))
        .on_conflict(username::tg_username)
        .do_update()
        .set((
            username::kind.eq(kind),
            username::tg_id.eq(Some(telegram_id)),
            username::updated_at.eq(now)
        ))
        .execute(&mut self.pool.get()?)?;

        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_username(&self, kind: UsernameKind, telegram_id: i64) -> Result<Option<String>, DatabaseError> {
        Ok(username::table
        .select( username::tg_username)
        .filter( username::kind.eq(kind))
        .filter( username::tg_id.eq(Some(telegram_id)))
        .first(&mut self.pool.get()?) .optional()?)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_usernames(&self, kind: UsernameKind, telegram_ids: Vec<i64>) -> Result<Vec<(i64, String)>, DatabaseError> {
        Ok(username::table
        .select((username::tg_id.assume_not_null(), username::tg_username))
        .filter( username::tg_id.is_not_null())
        .filter( username::kind.eq(kind))
        .filter( username::tg_id.eq_any(telegram_ids))
        .load(&mut self.pool.get()?)?)
    }
}
