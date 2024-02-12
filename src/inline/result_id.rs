use std::fmt::Display;

use nom::bytes::streaming::tag;
use nom::combinator::map;
use nom::sequence::preceded;
use nom::{branch::alt, IResult};

use crate::util::{sticker_id_literal, tag_literal};

#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) enum InlineQueryResultId {
    Sticker(String),
    Tag(String),
}

impl TryFrom<String> for InlineQueryResultId {
    type Error = anyhow::Error;

    fn try_from(input: String) -> Result<Self, Self::Error> {
        let (input, data) = parse_result(&input)
            .map_err(|err| anyhow::anyhow!("invalid callback data: {}", err))?;
        if input.is_empty() {
            Ok(data)
        } else {
            Err(anyhow::anyhow!("invalid callback data: input not consumed"))
        }
    }
}

fn parse_result(input: &str) -> IResult<&str, InlineQueryResultId> {
    alt((
        map(
            preceded(tag("s:"), sticker_id_literal),
            |sticker_unique_id| InlineQueryResultId::Sticker(sticker_unique_id.to_string()),
        ),
        map(preceded(tag("t:"), tag_literal), |tag| {
            InlineQueryResultId::Tag(tag.to_string())
        }),
    ))(input)
}

impl Display for InlineQueryResultId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sticker(id) => write!(f, "s:{id}"),
            Self::Tag(tag) => write!(f, "t:{tag}"),
        }
    }
}
