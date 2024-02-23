use thiserror::Error;

use crate::{background_tasks, database::DatabaseError, sticker::EmbeddingError};

#[derive(Error, Debug)]
pub enum BotError {
    #[error("teloxide error")]
    Teloxide(teloxide::RequestError),

    #[error("reqwest error")]
    Reqwest(reqwest::Error),

    #[error("database error")]
    Database(#[from] DatabaseError),

    #[error("embedding error")]
    Embedding(#[from] EmbeddingError),

    // TODO: add error infos
    #[error("a timeout occured")]
    Timeout, // TODO: convert all reqwest timeout errors to Timeout errors (instead of anyhow errors)

    // TODO: get rid of anyhow?
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<teloxide::RequestError> for BotError {
    fn from(value: teloxide::RequestError) -> Self {
        match value {
            teloxide::RequestError::Network(reqwest_err) => reqwest_err.into(),
            err => Self::Teloxide(err),
        }
    }
}

impl From<reqwest::Error> for BotError {
    fn from(value: reqwest::Error) -> Self {
        if value.is_timeout() {
            Self::Timeout
        } else {
            Self::Reqwest(value)
        }
    }
}

impl From<teloxide::DownloadError> for BotError {
    fn from(value: teloxide::DownloadError) -> Self {
        match value {
            teloxide::DownloadError::Network(reqwest_err) => reqwest_err.into(),
            err @ teloxide::DownloadError::Io(_) => Self::Teloxide(err.into()),
        }
    }
}

macro_rules! impl_other_error {
    ($error_type:ty) => {
        impl From<$error_type> for BotError {
            fn from(value: $error_type) -> Self {
                Self::Other(anyhow::anyhow!(value))
            }
        }
    };
}

impl_other_error!(std::io::Error);
impl_other_error!(serde_json::Error);
impl_other_error!(url::ParseError);
impl_other_error!(std::fmt::Error);
impl_other_error!(tokio::task::JoinError);
impl_other_error!(tokio::sync::oneshot::error::RecvError);
impl_other_error!(tokio::sync::mpsc::error::SendError<background_tasks::analysis::Command>);
impl_other_error!(tokio::sync::mpsc::error::SendError<background_tasks::tagging::TaggingWorkerCommand>);
