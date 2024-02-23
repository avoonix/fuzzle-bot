#[cfg(feature = "ssr")]
use nom::bytes::complete::tag;
#[cfg(feature = "ssr")]
use nom::combinator::{fail, map, success};
#[cfg(feature = "ssr")]
use nom::sequence::preceded;
#[cfg(feature = "ssr")]
use nom::IResult;
#[cfg(feature = "ssr")]
use nom::{branch::alt, character::complete::u64};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[cfg(feature = "ssr")]
use crate::database::StickerOrder;
#[cfg(feature = "ssr")]
use crate::util::{sticker_id_literal, tag_literal};

#[cfg(feature = "ssr")]
fn parse_tag_operation(input: &str) -> IResult<&str, TagOperation> {
    alt((
        map(preceded(tag("t"), parse_tag), TagOperation::Tag),
        map(preceded(tag("u"), parse_tag), TagOperation::Untag),
    ))(input)
}

#[cfg(feature = "ssr")]
fn parse_tag(input: &str) -> IResult<&str, String> {
    let (input, _) = tag(";")(input)?;
    let (input, tag) = tag_literal(input)?;
    Ok((input, tag.to_string()))
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
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

#[cfg(feature = "ssr")]
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
    ChangeSetStatus {
        set_name: String,
        banned: bool,
    },
    UserInfo(u64),
    SetOrder(StickerOrder),
    SetLock {
        lock: bool,
        sticker_id: String,
    },
}

#[cfg(feature = "ssr")]
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
    pub fn change_set_status(set_name: impl Into<String>, banned: bool) -> Self {
        Self::ChangeSetStatus {
            banned,
            set_name: set_name.into(),
        }
    }

    pub fn user_info(user_id: impl Into<u64>) -> Self {
        Self::UserInfo(user_id.into())
    }
}

#[cfg(feature = "ssr")]
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
        parse_order_data,
        parse_lock_data,
    ))(input)
}

#[cfg(feature = "ssr")]
fn parse_lock_data(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("lock;")(input)?;
    let (input, sticker_id) = sticker_id_literal(input)?;
    let (input, _) = tag(";")(input)?;
    alt((
        map(tag("lock"), |_| CallbackData::SetLock {
            lock: true,
            sticker_id: sticker_id.to_string(),
        }),
        map(tag("unlock"), |_| CallbackData::SetLock {
            lock: false,
            sticker_id: sticker_id.to_string(),
        }),
    ))(input)
}

#[cfg(feature = "ssr")]
fn parse_order_data(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("order")(input)?;
    let (input, _) = tag(";")(input)?;
    let order = serde_json::from_str(input);
    match order {
        Err(err) => return fail(input),
        Ok(order) => Ok(("", CallbackData::SetOrder(order))),
    }
}

#[cfg(feature = "ssr")]
fn parse_user_info_data(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("userinfo")(input)?;
    let (input, _) = tag(";")(input)?;
    let (input, user_id) = u64(input)?;
    Ok((input, CallbackData::UserInfo(user_id)))
}

#[cfg(feature = "ssr")]
fn parse_remove_set_data(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("chset;")(input)?;
    let (input, set_name) = tag_literal(input)?; // TODO: add separate set_name parser
    let (input, _) = tag(";")(input)?;
    let (input, banned) = alt((map(tag("ban"), |_| true), map(tag("unban"), |_| false)))(input)?;
    Ok((
        input,
        CallbackData::ChangeSetStatus {
            banned,
            set_name: set_name.to_string(),
        },
    ))
}

#[cfg(feature = "ssr")]
fn parse_remove_blacklist_data(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("removebl")(input)?;
    let (input, _) = tag(";")(input)?;
    let (input, tag) = tag_literal(input)?;
    Ok((input, CallbackData::RemoveBlacklistedTag(tag.to_string())))
}

#[cfg(feature = "ssr")]
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

#[cfg(feature = "ssr")]
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

#[cfg(feature = "ssr")]
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
            Self::ChangeSetStatus { set_name, banned } => {
                let action = if *banned { "ban" } else { "unban" };
                write!(f, "chset;{set_name};{action}")
            }
            Self::Info => write!(f, "info"),
            Self::UserInfo(user_id) => write!(f, "userinfo;{user_id}"),
            Self::SetOrder(order) => {
                let order = serde_json::to_string(order).unwrap_or_default();
                write!(f, "order;{order}")
            }
            Self::SetLock { lock, sticker_id } => {
                let lock = if *lock { "lock" } else { "unlock" };
                write!(f, "lock;{sticker_id};{lock}")
            }
        }
    }
}

#[cfg(feature = "ssr")]
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
