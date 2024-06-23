use nom::branch::alt;
use nom::bytes::complete::{tag, take_while1};
use nom::character::complete::alphanumeric1;
use nom::combinator::{fail, recognize, success};
use nom::multi::many1_count;
use nom::IResult;

use super::{parse_first_emoji, Emoji};

pub fn sticker_id_literal(input: &str) -> IResult<&str, &str> {
    recognize(many1_count(alt((alphanumeric1, tag("-"), tag("_")))))(input)
}

pub fn tag_literal(input: &str) -> IResult<&str, &str> {
    recognize(many1_count(alt((
        alphanumeric1,
        take_while1(|char: char| matches!(char, '_' | '-' | '(' | ')' | '/' | '<' | '>' | '!' | '?' | '\'' | ':' | '.' | '^')), // may not contain `,` due to TagList + syntax for defining multiple tags
    ))))(input)
}

pub fn set_name_literal(input: &str) -> IResult<&str, &str> {
    recognize(many1_count(alt((alphanumeric1, tag("_")))))(input)
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
    async fn parse_tag_literals_from_tag_manager() -> anyhow::Result<()> {
        let tag_manager = get_default_tag_manager(std::env::temp_dir()).await?;
        for tag in tag_manager.get_tags() {
            assert_eq!(Ok(("", tag.as_str())), tag_literal(&tag));
        }
        // go through all tags and verify that they parse


        // rules.emoji_rules.values().flatten().for_each(|tag| {
        // });

        Ok(())
    }
}
