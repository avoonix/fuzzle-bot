use std::sync::Arc;

use futures::future::BoxFuture;
use teloxide::prelude::*;
use teloxide::types::Recipient;

use crate::{bot::BotExt, text::Markdown};

use super::{Bot, BotError, InternalError};

#[derive(Debug)]
pub struct ErrorHandler;

impl ErrorHandler {
    #[must_use]
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self { })
    }
}

impl teloxide::error_handlers::ErrorHandler<()> for ErrorHandler {
    /// noop - errors are reported in handler wrappers
    fn handle_error(self: Arc<Self>, error: ()) -> BoxFuture<'static, ()> {
        Box::pin(async move {})
    }
}

pub fn report_periodic_task_error<T>(result: Result<T, InternalError>) {
    // errors are usually collected in jaeger
    match result {
        Ok(_) => {}
        Err(err) => tracing::error!("periodic task error: {err:?}")
    }
}

pub fn report_internal_error(result: &InternalError) {
    tracing::error!("handler error: {result:?}");
}

pub fn report_bot_error(result: &BotError) {
    match result {
        BotError::InternalError(error) => tracing::error!("handler error: {error:?}"),
        BotError::UserError(error) => {}
    }
}

pub fn report_internal_error_result<T>(result: Result<T, BotError>) {
    match result {
        Ok(_) => {},
        Err(BotError::InternalError(error)) => tracing::error!("handler error: {error:?}"),
        Err(BotError::UserError(error)) => {}
    }
}
