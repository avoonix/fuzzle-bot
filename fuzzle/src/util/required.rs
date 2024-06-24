use crate::bot::{InternalError, UserError};

pub trait Required<T> {
    fn required(self) -> Result<T, InternalError>;
}

impl<T> Required<T> for Option<T> {
    fn required(self) -> Result<T, InternalError> {
        match self {
            Some(value) => Ok(value),
            None => Err(InternalError::UnexpectedNone {
                type_name: std::any::type_name::<T>().to_string(),
            }),
        }
    }
}
