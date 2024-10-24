use actix_web::{body::BoxBody, HttpResponse, ResponseError};

use crate::{bot::{BotError, InternalError}, database::DatabaseError, qdrant::VectorDatabaseError};

impl ResponseError for BotError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::InternalServerError().finish() // TODO: better error for common user-facing errors
    }
}
impl ResponseError for InternalError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::InternalServerError().finish() // TODO: better error for common user-facing errors
    }
}
impl ResponseError for DatabaseError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::InternalServerError().finish() // TODO: better error for common user-facing errors
    }
}
impl ResponseError for VectorDatabaseError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::InternalServerError().finish() // TODO: better error for common user-facing errors
    }
}
