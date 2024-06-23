use diesel::{
    debug_query,
    query_builder::{AsQuery, BoxedSqlQuery, SqlQuery},
    sql_query,
    sql_types::{BigInt, Integer, Nullable, Text},
    sqlite::Sqlite,
    IntoSql,
};
use itertools::{Itertools, Position};

use super::Order;

/// stickers must be tagged to be found (even if you just query for emojis)
#[derive(Debug)]
pub(super) struct StickerTagQuery {
    must: Vec<String>,
    must_not: Vec<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    order: Option<Order>,
    emoji: Vec<String>,
}

impl StickerTagQuery {
    #[must_use]
    pub(super) fn new(must: Vec<String>, must_not: Vec<String>) -> Self {
        Self {
            must,
            must_not,
            limit: None,
            offset: None,
            order: None,
            emoji: vec![],
        }
    }

    #[must_use]
    pub(super) const fn limit(mut self, limit: i64) -> Self {
        self.limit = Some(limit);
        self
    }

    #[must_use]
    pub(super) const fn offset(mut self, offset: i64) -> Self {
        self.offset = Some(offset);
        self
    }

    #[must_use]
    pub(super) const fn order(mut self, order: Order) -> Self {
        self.order = Some(order);
        self
    }

    #[must_use]
    pub(super) fn emoji(mut self, emoji: Vec<String>) -> Self {
        self.emoji = emoji;
        self
    }

    #[must_use]
    pub(super) fn generate(&self) -> BoxedSqlQuery<Sqlite, SqlQuery> {
        // TODO: test if exists or in is faster:
        // exists:
        //  EXISTS(SELECT * FROM sticker_file_tag AS rel WHERE rel.sticker_file = sticker_file_tag.sticker_file AND tag = 'duo')
        //  sticker_file_tag.sticker_file in (select sticker_file from sticker_file_tag where tag = 'solo')
        // in:
        //  query_builder.push("WHERE sticker_file IN (SELECT sticker_file FROM sticker_file_tag WHERE ");
        //  let mut separated = query_builder.separated(" AND ");
        //  for tag in &self.must {
        //      separated.push(
        //          "sticker_file_tag.sticker_file IN (SELECT sticker_file FROM sticker_file_tag WHERE tag = ",
        //      );
        //      separated.push_bind_unseparated(tag);
        //      separated.push_unseparated(")");
        //  }

        let mut q = sql_query("").into_boxed::<Sqlite>()
            .sql("SELECT * FROM sticker ")
            .sql("WHERE sticker.sticker_file_id IN (SELECT sticker_file_id FROM sticker_file_tag GROUP BY sticker_file_id ");

        // https://stackoverflow.com/a/69911488
        // TODO: benchmark if HAVING is faster than the old IN or EXISTS variants
        if self.must.len() > 0 || self.must_not.len() > 0 {
            q = q.sql("HAVING ");
        }

        if self.must.len() > 0 {
            q = q.sql("count(CASE WHEN tag IN ");
            q = generate_sql_list(q, self.must.clone());
            q = q
                .sql(" THEN 1 END) = ?")
                .bind::<Integer, _>(self.must.len() as i32);
        }

        if self.must.len() > 0 && self.must_not.len() > 0 {
            q = q.sql(" AND ")
        }

        if self.must_not.len() > 0 {
            q = q.sql("count(CASE WHEN tag IN ");
            q = generate_sql_list(q, self.must_not.clone());
            q = q.sql(" THEN 1 END) = 0")
        }

        q = q .sql(") ");

        if self.emoji.len() > 0 {
            q = q.sql("AND sticker.emoji IN ");
            q = generate_sql_list(q, self.emoji.clone());
            q = q.sql(" ");
        }

        q = q
            .sql("GROUP BY sticker.sticker_file_id ")
            .sql("ORDER BY ");
        q = match self.order {
            None | Some(Order::LatestFirst) => q.sql("rowid DESC "),
            Some(Order::Random { seed }) => {
                q.sql("sin(rowid + ?").bind::<Integer, _>(seed).sql(") ")
            } // sqlite doesn't support seeded random sort natively
        };

        let q = q
            .sql("LIMIT ? ")
            .bind::<BigInt, _>(self.limit.unwrap_or(10))
            .sql("OFFSET ?")
            .bind::<BigInt, _>(self.offset.unwrap_or_default());
        q
    }
}

fn generate_sql_list(
    mut q: BoxedSqlQuery<Sqlite, SqlQuery>,
    list: Vec<String>,
) -> BoxedSqlQuery<Sqlite, SqlQuery> {
    for (position, item) in list.into_iter().with_position() {
        q = match position {
            Position::First => q.sql("(?, ").bind::<Text, _>(item),
            Position::Middle => q.sql("?, ").bind::<Text, _>(item),
            Position::Last => q.sql("?)").bind::<Text, _>(item),
            Position::Only => q.sql("(?)").bind::<Text, _>(item),
        }
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_builder_1() {
        let query = StickerTagQuery::new(vec!["solo".into()], vec!["meta_sticker".into()])
            .limit(100)
            .offset(200)
            .order(Order::LatestFirst);
        assert_eq!(&debug_query(&query.generate()).to_string(), "SELECT * FROM sticker WHERE sticker.sticker_file_id IN (SELECT sticker_file_id FROM sticker_file_tag GROUP BY sticker_file_id HAVING count(CASE WHEN tag IN (?) THEN 1 END) = ? AND count(CASE WHEN tag IN (?) THEN 1 END) = 0) GROUP BY sticker.sticker_file_id ORDER BY rowid DESC LIMIT ? OFFSET ? -- binds: [\"solo\", 1, \"meta_sticker\", 100, 200]");
        // assert_eq!(query.sql(), "SELECT * FROM sticker WHERE sticker_file IN (SELECT sticker_file FROM sticker_file_tag WHERE sticker_file_tag.sticker_file IN (SELECT sticker_file FROM sticker_file_tag WHERE tag = ?) AND sticker_file_tag.sticker_file NOT IN (SELECT sticker_file FROM sticker_file_tag WHERE tag = ?)) GROUP BY sticker.sticker_file_id ORDER BY rowid DESC LIMIT ? OFFSET ?");
    }

    #[test]
    fn test_query_builder_2() {
        let query = StickerTagQuery::new(
            vec!["solo".into(), "heart_symbol".into()],
            vec!["meta_sticker".into()],
        )
        .limit(100)
        .offset(200)
        .emoji(vec!["🤍".to_string(), "💚".to_string()])
        .order(Order::Random { seed: 42 });
        assert_eq!(&debug_query(&query.generate()).to_string(), "SELECT * FROM sticker WHERE sticker.sticker_file_id IN (SELECT sticker_file_id FROM sticker_file_tag GROUP BY sticker_file_id HAVING count(CASE WHEN tag IN (?, ?) THEN 1 END) = ? AND count(CASE WHEN tag IN (?) THEN 1 END) = 0) AND sticker.emoji IN (?, ?) GROUP BY sticker.sticker_file_id ORDER BY sin(rowid + ?) LIMIT ? OFFSET ? -- binds: [\"solo\", \"heart_symbol\", 2, \"meta_sticker\", \"🤍\", \"💚\", 42, 100, 200]");
        // assert_eq!(query.sql(), "SELECT * FROM sticker WHERE sticker_file IN (SELECT sticker_file FROM sticker_file_tag WHERE sticker_file_tag.sticker_file IN (SELECT sticker_file FROM sticker_file_tag WHERE tag = ?) AND sticker_file_tag.sticker_file IN (SELECT sticker_file FROM sticker_file_tag WHERE tag = ?) AND sticker_file_tag.sticker_file NOT IN (SELECT sticker_file FROM sticker_file_tag WHERE tag = ?)) GROUP BY sticker.sticker_file_id ORDER BY sin(rowid + ?) LIMIT ? OFFSET ?");
    }

    #[test]
    fn test_query_builder_3() {
        let query = StickerTagQuery::new(
            vec![],
            vec!["meta_sticker".into()],
        )
        .limit(100)
        .offset(200)
        .emoji(vec!["🤍".to_string()])
        .order(Order::Random { seed: 42 });
        assert_eq!(&debug_query(&query.generate()).to_string(), "SELECT * FROM sticker WHERE sticker.sticker_file_id IN (SELECT sticker_file_id FROM sticker_file_tag GROUP BY sticker_file_id HAVING count(CASE WHEN tag IN (?) THEN 1 END) = 0) AND sticker.emoji IN (?) GROUP BY sticker.sticker_file_id ORDER BY sin(rowid + ?) LIMIT ? OFFSET ? -- binds: [\"meta_sticker\", \"🤍\", 42, 100, 200]");
    }

    #[test]
    fn test_query_builder_4() {
        let query = StickerTagQuery::new(
            vec![],
            vec![],
        )
        .limit(100)
        .offset(200)
        .emoji(vec!["🤍".to_string(), "💚".to_string()])
        .order(Order::Random { seed: 42 });
        assert_eq!(&debug_query(&query.generate()).to_string(), "SELECT * FROM sticker WHERE sticker.sticker_file_id IN (SELECT sticker_file_id FROM sticker_file_tag GROUP BY sticker_file_id ) AND sticker.emoji IN (?, ?) GROUP BY sticker.sticker_file_id ORDER BY sin(rowid + ?) LIMIT ? OFFSET ? -- binds: [\"🤍\", \"💚\", 42, 100, 200]");
    }

    #[test]
    fn test_query_builder_5() {
        let query = StickerTagQuery::new(
            vec![],
            vec!["meta_sticker".to_string()],
        )
        .limit(100)
        .offset(200)
        .order(Order::Random { seed: 42 });
        assert_eq!(&debug_query(&query.generate()).to_string(), "SELECT * FROM sticker WHERE sticker.sticker_file_id IN (SELECT sticker_file_id FROM sticker_file_tag GROUP BY sticker_file_id HAVING count(CASE WHEN tag IN (?) THEN 1 END) = 0) GROUP BY sticker.sticker_file_id ORDER BY sin(rowid + ?) LIMIT ? OFFSET ? -- binds: [\"meta_sticker\", 42, 100, 200]");
    }
}
