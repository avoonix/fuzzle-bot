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
            // ensure that used sticker sets are always kept updated
            request_context
                .process_set_of_sticker(sticker_unique_id.clone())
                .await;

            request_context.database
                .add_recently_used_sticker(request_context.user.id, &sticker_unique_id)
                .await?;
        }
        InlineQueryResultId::Tag(tag) => {}
        InlineQueryResultId::Set(set_id) => {}
        InlineQueryResultId::Emoji(emoji) => {}
        InlineQueryResultId::Other(description) => {
            tracing::error!("some user clicked on the {description} message");
            // TODO: show the user a "you were not supposed to click this" message?
        }
    }

    Ok(())
}
