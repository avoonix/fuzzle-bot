use teloxide::prelude::*;

use crate::{
    background_tasks::BackgroundTaskExt, bot::{report_internal_error, BotError, InternalError, RequestContext}
};

use super::result_id::InlineQueryResultId;

#[tracing::instrument(skip(request_context))]
pub async fn inline_result_handler_wrapper(
    q: ChosenInlineResult,
    request_context: RequestContext,
) -> Result<(), ()> {
    match inline_result_handler(q, request_context).await {
        Ok(_) => {},
        Err(error) => report_internal_error(&error)
    }
    Ok(())
}

#[tracing::instrument(skip(request_context, q), err(Debug))]
pub async fn inline_result_handler(
    q: ChosenInlineResult,
    request_context: RequestContext,
) -> Result<(), InternalError> {
    let result = InlineQueryResultId::try_from(q.result_id)?;
    match result {
        InlineQueryResultId::Sticker(sticker_unique_id) => {
            request_context.database
                .add_recently_used_sticker(request_context.user.id, &sticker_unique_id)
                .await?;
        }
        InlineQueryResultId::Tag(tag) => {}
        InlineQueryResultId::Emoji(emoji) => {}
        InlineQueryResultId::User(user_id) => {}
        InlineQueryResultId::Other(description) => {}
    }

    Ok(())
}
