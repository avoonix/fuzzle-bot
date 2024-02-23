use teloxide::prelude::*;

use crate::{
    background_tasks::{BackgroundTaskExt}, bot::{BotError, RequestContext, UserMeta}, database::Database
};

use super::result_id::InlineQueryResultId;

pub async fn inline_result_handler(
    q: ChosenInlineResult,
    request_context: RequestContext,
) -> Result<(), BotError> {
    // user: UserMeta,
    // database: Database,
    let result = InlineQueryResultId::try_from(q.result_id)?;
    match result {
        InlineQueryResultId::Sticker(sticker_unique_id) => {
            // ensure that used sticker sets are always kept updated
            request_context
                .process_set_of_sticker(sticker_unique_id.clone())
                .await;

            request_context.database
                .add_recently_used_sticker(request_context.user_id().0, sticker_unique_id)
                .await?;
        }
        InlineQueryResultId::Tag(tag) => {}
    }

    Ok(())
}
