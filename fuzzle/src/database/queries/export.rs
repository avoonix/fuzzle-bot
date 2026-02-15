use super::super::schema::*;
use super::Database;
use super::DatabaseError;
use diesel::prelude::*;
use tracing::warn;

impl Database {
    /// returns (sticker_file_id, tag)
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn export_file_tagged_tag_relationships(
        &self,
    ) -> Result<Vec<(String, String)>, DatabaseError> {
        self
            .exec(move |conn| {
                Ok(sticker_file_tag::table
                    .select((sticker_file_tag::sticker_file_id, sticker_file_tag::tag))
                    .load(conn)?)
            })
            .await
    }

    /// returns (sticker_id, sticker_file_id)
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn export_sticker_is_a_file_relationship(
        &self,
    ) -> Result<Vec<(String, String)>, DatabaseError> {
        self
            .exec(move |conn| {
                Ok(sticker::table
                    .select((sticker::id, sticker::sticker_file_id))
                    .load(conn)?)
            })
            .await
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn export_set_contains_sticker_relationship(
        &self,
    ) -> Result<Vec<(String, String)>, DatabaseError> {
        self
            .exec(move |conn| {
                Ok(sticker::table
                    .select((sticker::sticker_set_id, sticker::id))
                    .load(conn)?)
            })
            .await
    }
}
