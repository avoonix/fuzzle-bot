use teloxide::utils::command::ParseError;
use thiserror::Error;

use crate::{background_tasks, database::DatabaseError, qdrant::VectorDatabaseError};

// BotError inludes internal (like database errors) and user-facing errors (like invalid syntax for input); InternalError only has internal errros


#[derive(Error, Debug)]
pub enum InternalError {
    #[error("teloxide error")]
    Teloxide(#[from] teloxide::RequestError),

    #[error("teloxide download error")]
    Download(#[from] teloxide::DownloadError),

    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),

    #[error("database error: {}", .0)]
    Database(#[from] DatabaseError),

    #[error("vector database error: {}", .0)]
    VectorDatabase(#[from] VectorDatabaseError),

    #[error("grpc error")]
    Grpc(#[from] tonic::Status),

    #[error("grpc transport error")]
    GrpcTransport(#[from] tonic::transport::Error),

    #[error("image error")]
    Image(#[from] image::ImageError),

    #[error("unexpected none")]
    UnexpectedNone {
        type_name: String
    },

    // TODO: get rid of anyhow?
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl InternalError {
    pub fn is_timeout_error(&self) -> bool {
        match self {
            Self::Reqwest(reqwest_error) => reqwest_error.is_timeout(),
            Self::Teloxide(teloxide::RequestError::Network(network_error)) => network_error.is_timeout(),
            Self::Download(teloxide::DownloadError::Network(network_error)) => network_error.is_timeout(),
            _ => false,
        }
    }
}

#[derive(Error, Debug)]
pub enum UserError {
    #[error("user is not allowed to execute {}", .0)]
    NoPermissionForAction(String),

    #[error("user is in invalid mode")]
    InvalidMode,

    #[error("user sent an unsupported sticker type")]
    UnsupportedStickerType,

    #[error("user sent a sticker that is not part of a set")]
    StickerNotPartOfSet,

    #[error("user sent something other than a sticker or text message")]
    UnhandledMessageType,

    #[error("command parse error")]
    CommandError(ParseError),

    #[error("inline input parse error")]
    ParseError(usize, String),

    #[error("tags did not closely match existing tags")]
    TagsNotFound(Vec<String>)
}

impl InternalError {
    pub fn end_user_error(&self) -> String {
        "Aw, something's wrong. ".to_string()
    }
}

impl UserError {
    pub fn end_user_error(&self) -> String {
        match self {
            UserError::NoPermissionForAction(action) => format!("Oh no, I can't let you do this ({action}) yet."),
            UserError::InvalidMode => "Can't do that here!".to_string(),
            UserError::UnsupportedStickerType => "Those don't look like regular stickers.".to_string(),
            UserError::StickerNotPartOfSet => "Stickers must be part of a set!".to_string(),
            UserError::UnhandledMessageType => "I have no idea what to do with this. Send me text messages containing commands, stickers, or t.me/addsticker links!".to_string(),
            UserError::CommandError(ParseError::UnknownCommand(input)) => format!("What the heck is a \"{input}\"?"),
            UserError::CommandError(error) => "Invalid arguments!".to_string(),
            UserError::ParseError(position, rest) => format!("Invalid input at position {position}: {}", rest.chars().take(10).collect::<String>()),
            UserError::TagsNotFound(tags) => format!("Could not find tags: {}", tags.join(", ")),
        }
    }
}

impl BotError {
    pub fn end_user_error(&self) -> String {
        match self {
            BotError::InternalError(err) => err.end_user_error(),
            BotError::UserError(err) => err.end_user_error(),
        }
    }
}

#[derive(Error, Debug)]
pub enum BotError {
    #[error("internal error")]
    InternalError(#[from] InternalError),

    #[error("user error")]
    UserError(#[from] UserError),
}

macro_rules! impl_other_error {
    ($error_type:ty) => {
        impl From<$error_type> for BotError {
            fn from(value: $error_type) -> Self {
                Self::InternalError(InternalError::Other(anyhow::anyhow!(value)))
            }
        }

        impl From<$error_type> for InternalError {
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

macro_rules! impl_error {
    ($error_type:ty, $internal_name:ident) => {
        impl From<$error_type> for BotError {
            fn from(value: $error_type) -> Self {
                Self::InternalError(InternalError::$internal_name(value))
            }
        }
    };
}

impl_error!(DatabaseError, Database);
impl_error!(VectorDatabaseError, VectorDatabase);
impl_error!(tonic::Status, Grpc);
impl_error!(tonic::transport::Error, GrpcTransport);
impl_error!(image::ImageError, Image);
impl_error!(teloxide::RequestError, Teloxide);
impl_error!(reqwest::Error, Reqwest);
impl_error!(anyhow::Error, Other);
impl_error!(teloxide::DownloadError, Download);
