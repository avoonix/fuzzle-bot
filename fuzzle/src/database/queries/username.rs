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
use crate::database::TagData;
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
            username::kind.eq(kind.to_i64()), // TODO: to_i64 should not be necessary
            username::tg_id.eq(Some(telegram_id)),
        ))
        .on_conflict(username::tg_username)
        .do_update()
        .set((
            username::kind.eq(kind.to_i64()), // TODO: to_i64 should not be necessary
            username::tg_id.eq(Some(telegram_id)),
            username::updated_at.eq(now)
        ))
        .execute(&mut self.pool.get()?)?;

        Ok(())
    }
}
