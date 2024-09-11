use diesel::delete;
use diesel::dsl::count_star;
use diesel::dsl::exists;
use diesel::dsl::not;
use diesel::dsl::sql;
use diesel::insert_into;
use diesel::prelude::*;
use diesel::r2d2::ConnectionManager;
use diesel::sql_query;
use diesel::sql_types::BigInt;
use diesel::sql_types::Text;
use diesel::update;
use itertools::Itertools;
use r2d2::PooledConnection;
use std::collections::HashMap;
use teloxide::types::ChatId;
use teloxide::types::UserId;

use crate::database::model::PopularTag;
use crate::database::ModerationTask;
use crate::database::ModerationTaskDetails;
use crate::database::ModerationTaskStatus;
use crate::database::Tag;
use crate::database::UserStats;
use crate::tags::Category;
use crate::util::Emoji;

use super::sticker::max;
use super::DatabaseError;

use super::Database;

use super::super::schema::*;

impl Database {
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn create_moderation_task(
        &self,
        details: &ModerationTaskDetails,
        created_by_user_id: i64,
    ) -> Result<(), DatabaseError> {
        insert_into(moderation_task::table)
            .values((
                moderation_task::details.eq(details),
                moderation_task::created_by_user_id.eq(created_by_user_id),
                moderation_task::completion_status.eq(ModerationTaskStatus::Pending),
            ))
            .execute(&mut self.pool.get()?)?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn change_moderation_task_status(&self, task_id: i64, status: ModerationTaskStatus) -> Result<ModerationTask, DatabaseError> {
        Ok(update(moderation_task::table)
            .filter(moderation_task::id.eq(task_id))
            .set(moderation_task::completion_status.eq(status))
            .returning(ModerationTask::as_select())
            .get_result(&mut self.pool.get()?)?)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_open_moderation_tasks(&self) -> Result<Vec<ModerationTask>, DatabaseError> {
        Ok(moderation_task::table
            .select(ModerationTask::as_select())
            .filter(moderation_task::completion_status.eq(ModerationTaskStatus::Pending))
            .load(&mut self.pool.get()?)?)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn get_moderation_task_by_id(&self, moderation_task_id: i64) -> Result<Option<ModerationTask>, DatabaseError> {
        Ok(moderation_task::table
            .select(ModerationTask::as_select())
            .filter(moderation_task::id.eq(moderation_task_id))
            .first(&mut self.pool.get()?).optional()?)
    }
}
