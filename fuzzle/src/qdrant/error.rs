use std::any::Any;

use itertools::Itertools;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VectorDatabaseError {
    #[error("vector missing: {}", .0)]
    MissingVector(String),
    // #[error("user permission error")]
    // NoPermissionForAction(String),
    // TODO: get rid of anyhow?
    // #[error(transparent)]
    // Other(#[from] anyhow::Error),
    #[error("other: {}", .0)]
    Other(anyhow::Error), // TODO: remove this error?
}

pub trait SensibleQdrantErrorExt<T> {
    fn convert_to_sensible_error(self) -> Result<Option<T>, VectorDatabaseError>;
}

impl<T> SensibleQdrantErrorExt<T> for anyhow::Result<T> {
    fn convert_to_sensible_error(self) -> Result<Option<T>, VectorDatabaseError> {
        match self {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                let error_message = err.to_string(); // TODO: is there some way to downcast this instead???
                let not_found_re = regex::RegexBuilder::new(r"No point with id .+ found")
                    .case_insensitive(true)
                    .build()
                    .expect("static regex to compile");
                if not_found_re.is_match(&error_message) {
                    tracing::info!("treating not found error as optional");
                    Ok(None)
                } else {
                    tracing::info!("treating error as actual error");
                    Err(VectorDatabaseError::Other(err))
                }
            }
        }
    }
}

impl From<anyhow::Error> for VectorDatabaseError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err)
    }
}
