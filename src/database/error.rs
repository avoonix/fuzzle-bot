// TODO: use thiserror
#[derive(Debug)]
pub enum DatabaseError {
    NoRowsAffected,
    TryingToInsertRemovedSet,
    Anyhow(anyhow::Error),
    Serde(serde_json::Error),
    Sqlx(sqlx::Error),
}

impl From<sqlx::Error> for DatabaseError {
    fn from(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::Database(database_error) => match database_error.message() {
                "trying_to_insert_removed_set" => Self::TryingToInsertRemovedSet,
                _ => Self::Sqlx(sqlx::Error::Database(database_error)),
            },
            error => Self::Sqlx(error),
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

impl std::error::Error for DatabaseError {}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlx(error) => write!(f, "sqlx error: {error}"),
            Self::NoRowsAffected => write!(f, "no rows affected"),
            Self::TryingToInsertRemovedSet => write!(f, "trying to insert removed set"),
            Self::Anyhow(error) => write!(f, "anyhow error: {error}"),
            Self::Serde(error) => write!(f, "serde error: {error}"),
        }
    }
}
