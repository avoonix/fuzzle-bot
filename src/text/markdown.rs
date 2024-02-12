use std::fmt::Display;

use teloxide::utils::markdown::escape;

#[derive(Debug)]
pub struct Markdown(String);

impl Markdown {
    pub(super) fn new<T>(content: T) -> Self
    where
        T: Into<String>,
    {
        // TODO: should error if the message is above 4096 characters long (telegram message limit)
        Self(content.into())
    }

    pub fn escaped<T>(content: T) -> Self
    where
        T: Into<String>,
    {
        Self(escape(&content.into()))
    }
}

impl Display for Markdown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<Markdown> for String {
    fn from(value: Markdown) -> Self {
        value.to_string()
    }
}
