// TODO: use thiserror
#[derive(Debug)]
pub enum DatabaseError {
    NoRowsAffected,
    TryingToInsertRemovedSet,
    Anyhow(anyhow::Error),
    Serde(serde_json::Error),
    Diesel(diesel::result::Error),
    R2d2(r2d2::Error),
}

impl From<diesel::result::Error> for DatabaseError {
    fn from(error: diesel::result::Error) -> Self {
        Self::Diesel(error)
    }
}

impl From<r2d2::Error> for DatabaseError {
    fn from(value: r2d2::Error) -> Self {
        Self::R2d2(value)
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
            Self::Diesel(error) => write!(f, "diesel error: {error:?}"),
            Self::NoRowsAffected => write!(f, "no rows affected"),
            Self::TryingToInsertRemovedSet => write!(f, "trying to insert removed set"),
            Self::Anyhow(error) => write!(f, "anyhow error: {error}"),
            Self::Serde(error) => write!(f, "serde error: {error}"),
            Self::R2d2(error) => write!(f, "database pool error: {error}"),
        }
    }
}
