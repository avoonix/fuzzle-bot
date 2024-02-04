use sqlx::{QueryBuilder, Sqlite};

#[derive(Debug)]
pub(super) struct StickerTagQuery {
    must: Vec<String>,
    must_not: Vec<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

impl StickerTagQuery {
    #[must_use]
    pub(super) fn new(must: Vec<String>, must_not: Vec<String>) -> Self {
        Self {
            must,
            must_not,
            limit: None,
            offset: None,
        }
    }

    #[must_use]
    pub(super) const fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub(super) const fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    // bad performance, but at least makes use of (the limited) indexes
    #[must_use]
    pub(super) fn generate(&self) -> QueryBuilder<'_, Sqlite> {
        // TODO: test if exists or in is faster:
        // EXISTS(SELECT * FROM file_hash_tag AS rel WHERE rel.file_hash = file_hash_tag.file_hash AND tag = 'duo')
        // file_hash_tag.file_hash in (select file_hash from file_hash_tag where tag = 'solo')

        let mut query_builder: QueryBuilder<'_, Sqlite> =
            QueryBuilder::new("SELECT * FROM sticker ");
        query_builder.push("WHERE file_hash IN (SELECT file_hash FROM file_hash_tag WHERE ");
        let mut separated = query_builder.separated(" AND ");
        for tag in &self.must {
            separated.push(
                "file_hash_tag.file_hash IN (SELECT file_hash FROM file_hash_tag WHERE tag = ",
            );
            separated.push_bind_unseparated(tag);
            separated.push_unseparated(")");
        }
        for tag in &self.must_not {
            separated.push(
                "file_hash_tag.file_hash NOT IN (SELECT file_hash FROM file_hash_tag WHERE tag = ",
            );
            separated.push_bind_unseparated(tag);
            separated.push_unseparated(")");
        }

        query_builder.push(") GROUP BY sticker.file_hash LIMIT ");
        let limit = self.limit.unwrap_or(10) as i64;
        query_builder.push_bind(limit);

        query_builder.push(" OFFSET ");
        let offset = self.offset.unwrap_or(0) as i64;
        query_builder.push_bind(offset);

        query_builder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_builder_1() {
        let query = StickerTagQuery::new(vec!["solo".into()], vec!["meta_sticker".into()])
            .limit(100)
            .offset(200);
        let query = query.generate();
        assert_eq!(query.sql(), "SELECT * FROM sticker WHERE file_hash IN (SELECT file_hash FROM file_hash_tag WHERE file_hash_tag.file_hash IN (SELECT file_hash FROM file_hash_tag WHERE tag = ?) AND file_hash_tag.file_hash NOT IN (SELECT file_hash FROM file_hash_tag WHERE tag = ?)) GROUP BY sticker.file_hash LIMIT ? OFFSET ?");
        // TODO: check the bound values
    }

    #[test]
    fn test_query_builder_2() {
        let query = StickerTagQuery::new(
            vec!["solo".into(), "<3".into()],
            vec!["meta_sticker".into()],
        )
        .limit(100)
        .offset(200);
        let query = query.generate();
        assert_eq!(query.sql(), "SELECT * FROM sticker WHERE file_hash IN (SELECT file_hash FROM file_hash_tag WHERE file_hash_tag.file_hash IN (SELECT file_hash FROM file_hash_tag WHERE tag = ?) AND file_hash_tag.file_hash IN (SELECT file_hash FROM file_hash_tag WHERE tag = ?) AND file_hash_tag.file_hash NOT IN (SELECT file_hash FROM file_hash_tag WHERE tag = ?)) GROUP BY sticker.file_hash LIMIT ? OFFSET ?");
        // TODO: check the bound values
    }
}
