use deadpool::managed::PoolError;
use deadpool_diesel::InteractError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("unique constraint violated")]
    UniqueConstraintViolated(String),
    
    // TODO: these errors are not really database errors, rather application-level errors -> pollutes error log unnecessarily because it is expected that users submit banned stickers and they will simply be informed
    #[error("trying to insert removed set")]
    TryingToInsertRemovedSet,
    #[error("trying to insert removed sticker")]
    TryingToInsertRemovedSticker,
    #[error("no rows affected")]
    NoRowsAffected,

    #[error("other")]
    Anyhow(anyhow::Error),
    #[error("serde")]
    Serde(serde_json::Error),
    #[error("diesel")]
    Diesel(diesel::result::Error),
    #[error("deadpool")]
    Deadpool(#[from] deadpool::managed::PoolError<deadpool_diesel::Error>),
}

impl From<diesel::result::Error> for DatabaseError {
    fn from(error: diesel::result::Error) -> Self {
        match error {
            diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                error,
            ) => Self::UniqueConstraintViolated(error.message().to_string()),
            _ => Self::Diesel(error),
        }
    }
}

impl From<anyhow::Error> for DatabaseError {
    fn from(value: anyhow::Error) -> Self {
        Self::Anyhow(value)
    }
}

impl From<serde_json::Error> for DatabaseError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value)
    }
}

impl From<deadpool_diesel::InteractError> for DatabaseError {
    fn from(err: deadpool_diesel::InteractError) -> Self {
        match err {
            deadpool_diesel::InteractError::Panic(_) => 
                DatabaseError::Anyhow(anyhow::anyhow!("Database interaction panicked")),
            deadpool_diesel::InteractError::Aborted => 
                DatabaseError::Anyhow(anyhow::anyhow!("Database interaction aborted")),
        }
    }
}
