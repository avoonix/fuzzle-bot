use nom::bytes::complete::tag;
use nom::combinator::map;
use nom::sequence::preceded;
use nom::IResult;
use nom::{branch::alt, character::complete::u64};
use std::fmt::Display;

use crate::util::{sticker_id_literal, tag_literal};

fn parse_tag_operation(input: &str) -> IResult<&str, TagOperation> {
    alt((
        map(preceded(tag("t"), parse_tag), TagOperation::Tag),
        map(preceded(tag("u"), parse_tag), TagOperation::Untag),
    ))(input)
}

fn parse_tag(input: &str) -> IResult<&str, String> {
    let (input, _) = tag(";")(input)?;
    let (input, tag) = tag_literal(input)?;
    Ok((input, tag.to_string()))
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TagOperation {
    Tag(String),
    Untag(String),
}
impl Display for TagOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tag(tag) => write!(f, "t;{tag}"),
            Self::Untag(tag) => write!(f, "u;{tag}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CallbackData {
    Sticker {
        unique_id: String,
        operation: TagOperation,
    },
    Help,
    Start,
    Settings,
    Blacklist,
    RemoveBlacklistedTag(String),
    Info,
    RemoveSet(String),
    UserInfo(u64),
}

impl CallbackData {
    pub fn tag_sticker(unique_id: impl Into<String>, tag: impl Into<String>) -> Self {
        Self::Sticker {
            unique_id: unique_id.into(),
            operation: TagOperation::Tag(tag.into()),
        }
    }
    pub fn untag_sticker(unique_id: impl Into<String>, tag: impl Into<String>) -> Self {
        Self::Sticker {
            unique_id: unique_id.into(),
            operation: TagOperation::Untag(tag.into()),
        }
    }
    pub fn remove_set(set_name: impl Into<String>) -> Self {
        Self::RemoveSet(set_name.into())
    }

    pub fn user_info(user_id: impl Into<u64>) -> Self {
        Self::UserInfo(user_id.into())
    }
}

fn parse_callback_data(input: &str) -> IResult<&str, CallbackData> {
    alt((
        map(tag("start"), |_| CallbackData::Start),
        map(tag("settings"), |_| CallbackData::Settings),
        map(tag("help"), |_| CallbackData::Help),
        map(tag("blacklist"), |_| CallbackData::Blacklist),
        map(tag("info"), |_| CallbackData::Info),
        parse_remove_blacklist_data,
        parse_sticker_data,
        parse_remove_set_data,
        parse_user_info_data,
    ))(input)
}

fn parse_user_info_data(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("userinfo")(input)?;
    let (input, _) = tag(";")(input)?;
    let (input, user_id) = u64(input)?;
    Ok((input, CallbackData::UserInfo(user_id)))
}

fn parse_remove_set_data(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("rmset")(input)?;
    let (input, _) = tag(";")(input)?;
    let (input, tag) = tag_literal(input)?; // TODO: add separate set_name parser
    Ok((input, CallbackData::RemoveSet(tag.to_string())))
}

fn parse_remove_blacklist_data(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("removebl")(input)?;
    let (input, _) = tag(";")(input)?;
    let (input, tag) = tag_literal(input)?;
    Ok((input, CallbackData::RemoveBlacklistedTag(tag.to_string())))
}

fn parse_sticker_data(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("s")(input)?;
    let (input, _) = tag(";")(input)?;
    let (input, unique_id) = sticker_id_literal(input)?;
    let (input, _) = tag(";")(input)?;
    let (input, operation) = parse_tag_operation(input)?;
    Ok((
        input,
        CallbackData::Sticker {
            unique_id: unique_id.to_string(),
            operation,
        },
    ))
}

impl TryFrom<String> for CallbackData {
    type Error = anyhow::Error;

    fn try_from(input: String) -> Result<Self, Self::Error> {
        let (input, data) = parse_callback_data(&input)
            .map_err(|err| anyhow::anyhow!("invalid callback data: {}", err))?;
        if input.is_empty() {
            Ok(data)
        } else {
            Err(anyhow::anyhow!("invalid callback data: input not consumed"))
        }
    }
}

impl Display for CallbackData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sticker {
                unique_id,
                operation,
            } => write!(f, "s;{unique_id};{operation}"),
            Self::Help => write!(f, "help"),
            Self::Settings => write!(f, "settings"),
            Self::Start => write!(f, "start"),
            Self::Blacklist => write!(f, "blacklist"),
            Self::RemoveBlacklistedTag(tag) => write!(f, "removebl;{tag}"),
            Self::RemoveSet(set_name) => write!(f, "rmset;{set_name}"),
            Self::Info => write!(f, "info"),
            Self::UserInfo(user_id) => write!(f, "userinfo;{user_id}"),
        }
    }
}

impl From<CallbackData> for String {
    fn from(value: CallbackData) -> Self {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn parse_fail() {
        let result = CallbackData::try_from(String::new());
        assert!(result.unwrap_err().to_string().contains("Parsing Error"));
    }

    #[test]
    fn parse_sticker_tag() -> Result<()> {
        let data = CallbackData::try_from("s;5uh33fj84;t;male".to_string())?;
        assert_eq!(
            CallbackData::Sticker {
                unique_id: "5uh33fj84".to_string(),
                operation: TagOperation::Tag("male".to_string()),
            },
            data
        );
        Ok(())
    }

    #[test]
    fn parse_sticker_untag() -> Result<()> {
        let data = CallbackData::try_from("s;5uh33fj84;u;male".to_string())?;
        assert_eq!(
            CallbackData::Sticker {
                unique_id: "5uh33fj84".to_string(),
                operation: TagOperation::Untag("male".to_string()),
            },
            data
        );
        Ok(())
    }

    #[test]
    fn parse_stringify_help() -> Result<()> {
        let data = CallbackData::try_from("help".to_string())?;
        assert_eq!(data, CallbackData::Help);
        assert_eq!(data.to_string(), "help");
        Ok(())
    }

    #[test]
    fn parse_stringify_start() -> Result<()> {
        let data = CallbackData::try_from("start".to_string())?;
        assert_eq!(data, CallbackData::Start);
        assert_eq!(data.to_string(), "start");
        Ok(())
    }
}
