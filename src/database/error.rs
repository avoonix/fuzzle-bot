#[derive(Debug)]
pub enum DatabaseError {
    NoRowsAffected,
    TryingToInsertRemovedSet,
    Anyhow(anyhow::Error),
    Sqlx(sqlx::Error), // generic
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

impl std::error::Error for DatabaseError {}

impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlx(error) => write!(f, "sqlx error: {error}"),
            Self::NoRowsAffected => write!(f, "no rows affected"),
            Self::TryingToInsertRemovedSet => write!(f, "trying to insert removed set"),
            Self::Anyhow(error) => write!(f, "anyhow error: {error}")
        }
    }
}
