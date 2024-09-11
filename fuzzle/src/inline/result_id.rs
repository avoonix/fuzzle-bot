use std::fmt::Display;

use nom::bytes::streaming::tag;
use nom::character::complete::{alphanumeric1, i64};
use nom::combinator::{eof, map};
use nom::sequence::{preceded, terminated};
use nom::{branch::alt, IResult};

use crate::util::{parse_emoji, set_name_literal, sticker_id_literal, tag_literal, Emoji};

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) enum InlineQueryResultId {
    Sticker(String),
    Tag(String),
    Set(String),
    Emoji(Emoji),
    User(i64),
    Other(String),
}

impl TryFrom<String> for InlineQueryResultId {
    type Error = anyhow::Error;

    fn try_from(input: String) -> Result<Self, Self::Error> {
        let (input, data) = parse_result(&input)
            .map_err(|err| anyhow::anyhow!("invalid callback data: {}", err))?;
        Ok(data)
    }
}

fn parse_result(input: &str) -> IResult<&str, InlineQueryResultId> {
    terminated(alt((
        map(
            preceded(tag("s:"), sticker_id_literal),
            |sticker_unique_id| InlineQueryResultId::Sticker(sticker_unique_id.to_string()),
        ),
        map(preceded(tag("t:"), tag_literal), |tag| {
            InlineQueryResultId::Tag(tag.to_string())
        }),
        map(
            preceded(tag("st:"), set_name_literal),
            |set_id| InlineQueryResultId::Set(set_id.to_string()),
        ),
        map(
            preceded(tag("e:"), parse_emoji),
            |emoji| InlineQueryResultId::Emoji(emoji),
        ),
        map(
            preceded(tag("o:"), alphanumeric1),
            |description: &str| InlineQueryResultId::Other(description.to_string()),
        ),
        map(
            preceded(tag("u:"), i64),
            |user_id: i64| InlineQueryResultId::User(user_id),
        ),
    )), eof)(input)
}

impl Display for InlineQueryResultId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sticker(id) => write!(f, "s:{id}"),
            Self::Tag(tag) => write!(f, "t:{tag}"),
            Self::Set(set_id) => write!(f, "st:{set_id}"),
            Self::Emoji(emoji) => write!(f, "e:{}", emoji.to_string_without_variant()),
            Self::User(user_id) => write!(f, "u:{user_id}"),
            Self::Other(description) => write!(f, "o:{description}"),
        }
    }
}
