use nom::branch::alt;
use nom::bytes::complete::tag;
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
        tag("_"),
        tag("-"),
        tag("_"),
        tag("("),
        tag(")"),
        tag("/"),
        tag("<"),
    ))))(input)
}

pub fn set_name_literal(input: &str) -> IResult<&str, &str> {
    recognize(many1_count(alt((alphanumeric1, tag("_")))))(input)
}

pub fn parse_emoji(input: &str) -> IResult<&str, Emoji> {
    let (emoji, input) = parse_first_emoji(input);
    emoji.map_or_else(|| fail(input), |emoji| success(emoji)(input))
}
