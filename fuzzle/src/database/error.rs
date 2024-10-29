use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("no rows affected")]
    NoRowsAffected,
    #[error("unique constraint violated")]
    UniqueConstraintViolated(String),
    #[error("trying to insert removed set")]
    TryingToInsertRemovedSet,
    #[error("other")]
    Anyhow(anyhow::Error),
    #[error("serde")]
    Serde(serde_json::Error),
    #[error("diesel")]
    Diesel(diesel::result::Error),
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
