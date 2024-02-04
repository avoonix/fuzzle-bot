use teloxide::prelude::*;

use crate::{
    bot::{BotError, UserMeta}, database::Database, worker::WorkerPool
};

use super::result_id::InlineQueryResultId;

pub async fn inline_result_handler(
    q: ChosenInlineResult,
    worker: WorkerPool,
    user: UserMeta,
    database: Database
) -> Result<(), BotError> {
    let result = InlineQueryResultId::try_from(q.result_id)?;
    // let query = InlineQueryData::try_from(q.query.clone())?; // TODO: handle inline results
    // dbg!("inline result handler", q.from.id, q.result_id, query, q.inline_message_id);

    match result {
        InlineQueryResultId::Sticker(sticker_unique_id) => {
            // ensure that used sticker sets are always kept updated
            worker
                .process_set_of_sticker(Some(user.id()), sticker_unique_id.clone())
                .await;
            
            database.add_recently_used_sticker(user.id().0, sticker_unique_id, q.query).await?;
        }
        InlineQueryResultId::Tag(tag) => {}
    }

    Ok(())
}
