use nom::branch::alt;
use nom::bytes::complete::{tag, take_while1};
use nom::character::complete::{alpha1, alphanumeric1, satisfy};
use nom::combinator::{fail, recognize, success, opt};
use nom::multi::{many1, many1_count, separated_list1};
use nom::sequence::{preceded, tuple};
use nom::IResult;

use super::{parse_first_emoji, Emoji};

pub fn sticker_id_literal(input: &str) -> IResult<&str, &str> {
    let (input, result) = recognize(many1(alt((alphanumeric1, tag("-"), tag("_")))))(input)?;
    if result.len() >= 10 { // valid ids seem to have length 12-16; TODO: if you are sure there are no stickers with length < 12, set that as minimum
        success(result)(input)
    } else {
        fail(input)
    }
}

pub fn tag_literal(input: &str) -> IResult<&str, &str> {
    recognize(many1_count(alt((
        alphanumeric1,
        take_while1(|char: char| {
            matches!(
                char,
                '_' | '-' | '(' | ')' | '/' | '<' | '>' | '!' | '?' | '\'' | ':' | '.' | '^'
            )
        }), // may not contain `,` due to TagList + syntax for defining multiple tags
    ))))(input)
}

pub fn set_name_literal(input: &str) -> IResult<&str, &str> {
    recognize( tuple((satisfy(|c| c.is_ascii_alphabetic()), opt(tag("_")), separated_list1(tag("_"), alphanumeric1))))(input)
}

pub fn parse_emoji(input: &str) -> IResult<&str, Emoji> {
    let (emoji, input) = parse_first_emoji(input);
    emoji.map_or_else(|| fail(input), |emoji| success(emoji)(input))
}

#[cfg(test)]
mod tests {
    use crate::tags::get_default_tag_manager;

    use super::*;
    use anyhow::Result;
    use nom::Finish;

    #[tokio::test]
    async fn parse_set_names() -> anyhow::Result<()> {
        for name in ["asdf", "a_b_c", "a1234"] {
            assert_eq!(Ok(("", name)), set_name_literal(name).finish());
        }
        for name in ["_asdf_", "a__b", "1234a"] {
            assert!(matches!(set_name_literal(name).finish(), Err(_)));
        }
        Ok(())
    }

    #[tokio::test]
    async fn parse_tag_literals_from_tag_manager() -> anyhow::Result<()> {
        let tag_manager = get_default_tag_manager(std::env::temp_dir()).await?;
        for tag in tag_manager.get_tags() {
            assert_eq!(Ok(("", tag.as_str())), tag_literal(&tag));
        }
        Ok(())
    }
}
