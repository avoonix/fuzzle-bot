use actix_web::ResponseError;

use crate::{bot::{BotError, InternalError}, database::DatabaseError};

// TODO: DOES THIS EXPOSE INTERNAL DETAILS?
impl ResponseError for BotError {}
impl ResponseError for InternalError {}
impl ResponseError for DatabaseError {}
