use enum_primitive_derive::Primitive;
use itertools::Itertools;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_while;
use nom::character::complete::digit1;
use nom::character::complete::multispace0;
use nom::character::complete::multispace1;
use nom::combinator::eof;
use nom::combinator::map;
use nom::combinator::map_res;
use nom::combinator::opt;
use nom::error::ParseError;
use nom::multi::{many0, many1, separated_list0};
use nom::sequence::delimited;
use nom::sequence::tuple;
use nom::sequence::{preceded, terminated};
use nom::Finish;
use nom::IResult;
use nom::Parser;
use num_traits::FromPrimitive;
use serde_repr::Deserialize_repr;
use serde_repr::Serialize_repr;
use std::fmt::Display;
use std::str::FromStr;

use crate::bot::UserError;
use crate::util::{parse_emoji, set_name_literal, sticker_id_literal, tag_literal, Emoji};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SetOperation {
    Tag,
    Untag,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InlineQueryData {
    SearchTagsForSticker {
        tags: Vec<Vec<String>>,
        unique_id: String,
    },
    SearchTagsForStickerSet {
        tags: Vec<Vec<String>>,
        set_name: String,
        operation: SetOperation,
    },
    SearchTagsForContinuousTagMode {
        tags: Vec<Vec<String>>,
        operation: SetOperation,
    },
    SearchTagsForBlacklist {
        tags: Vec<String>,
    },
    SearchStickers {
        tags: Vec<String>,
        emoji: Vec<Emoji>,
    },
    ListAllTagsFromSet {
        sticker_id: String,
    },
    AddToUserSet {
        sticker_id: String,
        set_title: Option<String>,
    },
    ListAllSetsThatContainSticker {
        sticker_id: String,
    },
    ListOverlappingSets {
        sticker_id: String,
    },
    ListSetStickersByDate {
        sticker_id: String,
    },
    ListSimilarStickers {
        unique_id: String,
        aspect: SimilarityAspect,
    },
    ListMostDuplicatedStickers,
    ListMostUsedEmojis,
    ListRecommendationModeRecommendations,
    SearchByEmbedding {
        query: String,
    },
    SetsByUserId {
        user_id: i64,
    },
    TagCreatorTagId {
        tag_id: String,
        kind: TagKind,
    },
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Primitive)]
pub enum TagKind {
    #[default]
    Main = 0,
    Alias = 1,
}

impl FromStr for TagKind {
    type Err = UserError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s: u8 = s
            .parse()
            .map_err(|_| UserError::ParseError(0, s.to_string()))?;
        Ok(TagKind::from_u8(s).unwrap_or_default())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum SimilarityAspect {
    Color,
    // Shape, TODO: autoencoder?
    Embedding,
}

impl InlineQueryData {
    #[must_use]
    pub fn empty_sticker_query(sticker_unique_id: impl Into<String>) -> Self {
        Self::SearchTagsForSticker {
            unique_id: sticker_unique_id.into(),
            tags: vec![],
        }
    }

    #[must_use]
    pub fn sticker_query(sticker_unique_id: impl Into<String>, tags: Vec<Vec<String>>) -> Self {
        Self::SearchTagsForSticker {
            unique_id: sticker_unique_id.into(),
            tags,
        }
    }

    #[must_use]
    pub fn similar(sticker_unique_id: impl Into<String>, aspect: SimilarityAspect) -> Self {
        Self::ListSimilarStickers {
            unique_id: sticker_unique_id.into(),
            aspect,
        }
    }

    #[must_use]
    pub fn embedding(query: String) -> Self {
        Self::SearchByEmbedding { query }
    }

    #[must_use]
    pub fn blacklist_query(tags: Vec<String>) -> Self {
        Self::SearchTagsForBlacklist { tags }
    }

    #[must_use]
    pub fn set_operation(
        set_name: impl Into<String>,
        tags: Vec<Vec<String>>,
        operation: SetOperation,
    ) -> Self {
        Self::SearchTagsForStickerSet {
            set_name: set_name.into(),
            operation,
            tags,
        }
    }

    #[must_use]
    pub fn continuous_tag_mode(tags: Vec<Vec<String>>, operation: SetOperation) -> Self {
        Self::SearchTagsForContinuousTagMode { operation, tags }
    }

    #[must_use]
    pub fn tag_creator(tag_id: String, kind: TagKind) -> Self {
        Self::TagCreatorTagId { tag_id, kind }
    }

    #[must_use]
    pub fn search_emoji(tags: Vec<String>, emoji: Vec<Emoji>) -> Self {
        Self::SearchStickers { emoji, tags }
    }

    #[must_use]
    pub fn search(tags: Vec<String>) -> Self {
        Self::SearchStickers {
            emoji: vec![],
            tags,
        }
    }

    #[must_use]
    pub fn sets(sticker_id: String) -> Self {
        Self::ListAllSetsThatContainSticker { sticker_id }
    }

    #[must_use]
    pub fn overlapping_sets(sticker_id: String) -> Self {
        Self::ListOverlappingSets { sticker_id }
    }

    #[must_use]
    pub fn set_stickers_by_date(sticker_id: String) -> Self {
        Self::ListSetStickersByDate { sticker_id }
    }

    #[must_use]
    pub fn all_set_tags(sticker_id: String) -> Self {
        Self::ListAllTagsFromSet { sticker_id }
    }

    #[must_use]
    pub fn most_duplicated_stickers() -> Self {
        Self::ListMostDuplicatedStickers
    }

    #[must_use]
    pub fn most_used_emojis() -> Self {
        Self::ListMostUsedEmojis
    }

    #[must_use]
    pub fn recommendations() -> Self {
        Self::ListRecommendationModeRecommendations
    }

    #[must_use]
    pub fn add_to_user_set(sticker_id: String) -> Self {
        Self::AddToUserSet {
            sticker_id,
            set_title: None,
        }
    }
}

fn parse_tags(input: &str) -> IResult<&str, Vec<String>> {
    map(
        delimited(
            multispace0,
            separated_list0(multispace1, tag_literal),
            multispace0,
        ),
        |tags| tags.into_iter().map(|s| s.to_string()).collect_vec(),
    )(input)
}

fn parse_comma_separated_tags(input: &str) -> IResult<&str, Vec<Vec<String>>> {
    map(
        delimited(
            multispace0,
            separated_list0(tag(","), parse_tags),
            multispace0,
        ),
        |tags| tags.into_iter().filter(|t| !t.is_empty()).collect_vec(),
    )(input)
}

enum EmojiOrTag {
    Emoji(Emoji),
    Tag(String),
}

fn parse_tags_and_emojis(input: &str) -> IResult<&str, (Vec<String>, Vec<Emoji>)> {
    map(
        delimited(
            multispace0,
            separated_list0(
                multispace1,
                alt((
                    map(tag_literal, |tag| EmojiOrTag::Tag(tag.to_string())),
                    map(parse_emoji, |emoji| EmojiOrTag::Emoji(emoji)),
                )),
            ),
            multispace0,
        ),
        |entries| {
            let mut tags = Vec::new();
            let mut emojis = Vec::new();
            for entry in entries {
                match entry {
                    EmojiOrTag::Emoji(e) => emojis.push(e),
                    EmojiOrTag::Tag(t) => tags.push(t),
                }
            }
            (tags, emojis)
        },
    )(input)
}

// let (input, emoji) = opt(parse_emoji)(input)?;
// let (input, _) = many0(tag(" "))(input)?;
// let (input, emoji) = opt(parse_emoji)(input)?;
// let (input, _) = many0(tag(" "))(input)?;
// if matches!(mode, InlineQueryData::StickerSearch { .. }) {
//     mode = InlineQueryData::StickerSearch { emoji };
// }
// let (input, tags) = separated_list0(many1(tag(" ")), tag_literal)(input)?;
// Ok((
//     input,
//     InlineQueryData {
//         mode,
//         tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
//     },
// ))

fn parse_inline_query_data(input: &str) -> IResult<&str, InlineQueryData> {
    terminated(
        alt((
            map(
                tuple((
                    terminated(preceded(tag("(s:"), sticker_id_literal), tag(")")),
                    parse_comma_separated_tags,
                )),
                |(unique_id, tags)| InlineQueryData::SearchTagsForSticker {
                    unique_id: unique_id.to_string(),
                    tags,
                },
            ),
            map(
                tuple((
                    alt((
                        map(tag("(se:"), |_| SetOperation::Tag),
                        map(tag("(su:"), |_| SetOperation::Untag),
                    )),
                    set_name_literal,
                    tag(")"),
                    parse_comma_separated_tags,
                )),
                |(operation, set_name, _, tags)| InlineQueryData::SearchTagsForStickerSet {
                    set_name: set_name.to_string(),
                    operation,
                    tags,
                },
            ),
            map(
                terminated(preceded(tag("(contains:"), sticker_id_literal), tag(")")),
                |sticker_id| InlineQueryData::ListAllSetsThatContainSticker {
                    sticker_id: sticker_id.to_string(),
                },
            ),
            map(
                terminated(preceded(tag("(overlap:"), sticker_id_literal), tag(")")),
                |sticker_id| InlineQueryData::ListOverlappingSets {
                    sticker_id: sticker_id.to_string(),
                },
            ),
            map(
                terminated(preceded(tag("(dates:"), sticker_id_literal), tag(")")),
                |sticker_id| InlineQueryData::ListSetStickersByDate {
                    sticker_id: sticker_id.to_string(),
                },
            ),
            map(
                terminated(preceded(tag("(usersets:"), map_res(digit1, str::parse)), tag(")")),
                |user_id: i64| InlineQueryData::SetsByUserId {
                    user_id: user_id,
                },
            ),
            map(
                terminated(preceded(tag("(settags:"), sticker_id_literal), tag(")")),
                |sticker_id| InlineQueryData::ListAllTagsFromSet {
                    sticker_id: sticker_id.to_string(),
                },
            ),
            map(
                tuple((
                    tag("(add:"),
                    sticker_id_literal,
                    tag(")"),
                    take_while(|c| true),
                )),
                |(_, sticker_id, _, set_title)| InlineQueryData::AddToUserSet {
                    sticker_id: sticker_id.to_string(),
                    set_title: {
                        let title = set_title.trim();
                        if title.is_empty() {
                            None
                        } else {
                            Some(title.to_string())
                        }
                    },
                },
            ),
            map(preceded(tag("(blacklist)"), parse_tags), |tags| {
                InlineQueryData::SearchTagsForBlacklist { tags }
            }),
            map(
                preceded(tag("(tag)"), take_while(|c| true)),
                |tag_id: &str| InlineQueryData::TagCreatorTagId {
                    tag_id: tag_id.to_string(),
                    kind: TagKind::Main,
                },
            ),
            map(
                preceded(tag("(alias)"), take_while(|c| true)),
                |tag_id: &str| InlineQueryData::TagCreatorTagId {
                    tag_id: tag_id.to_string(),
                    kind: TagKind::Alias,
                },
            ),
            map(
                preceded(tag("(embed)"), take_while(|c| true)),
                |query: &str| InlineQueryData::SearchByEmbedding {
                    query: query.to_string(),
                },
            ),
            map(tag("(dup)"), |_| {
                InlineQueryData::ListMostDuplicatedStickers
            }),
            map(tag("(emo)"), |_| InlineQueryData::ListMostUsedEmojis),
            map(tag("(rec)"), |_| {
                InlineQueryData::ListRecommendationModeRecommendations
            }),
            map(
                tuple((
                    alt((
                        map(tag("(cont)"), |_| SetOperation::Tag),
                        map(tag("(-cont)"), |_| SetOperation::Untag),
                    )),
                    parse_comma_separated_tags,
                )),
                |(operation, tags)| InlineQueryData::SearchTagsForContinuousTagMode {
                    operation,
                    tags,
                },
            ),
            map(
                terminated(preceded(tag("(color:"), sticker_id_literal), tag(")")),
                |unique_id| InlineQueryData::ListSimilarStickers {
                    aspect: SimilarityAspect::Color,
                    unique_id: unique_id.to_string(),
                },
            ),
            // map(
            //     terminated(preceded(tag("(shape:"), sticker_id_literal), tag(")")),
            //     |unique_id| InlineQueryData::Similar {
            //         aspect: SimilarityAspect::Shape,
            //         unique_id: unique_id.to_string(),
            //     },
            // ),
            map(
                terminated(preceded(tag("(embed:"), sticker_id_literal), tag(")")),
                |unique_id| InlineQueryData::ListSimilarStickers {
                    aspect: SimilarityAspect::Embedding,
                    unique_id: unique_id.to_string(),
                },
            ),
            map(parse_tags_and_emojis, |(tags, emoji)| {
                InlineQueryData::SearchStickers { emoji, tags }
            }),
        )),
        tuple((multispace0, eof)),
    )(input)
}

impl TryFrom<String> for InlineQueryData {
    type Error = UserError;

    fn try_from(input: String) -> Result<Self, Self::Error> {
        let (input, data) = Finish::finish(parse_inline_query_data(&input)).map_err(|err| {
            UserError::ParseError(input.len() - err.input.len(), err.input.to_string())
        })?;
        Ok(data)
    }
}

impl Display for InlineQueryData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            InlineQueryData::SearchTagsForSticker { unique_id, tags } => {
                let tags = tags.into_iter().map(|t| t.join(" ")).join(", ");
                write!(f, "(s:{unique_id}) {tags}")
            }
            InlineQueryData::ListAllSetsThatContainSticker { sticker_id } => {
                write!(f, "(contains:{sticker_id}) ")
            }
            InlineQueryData::ListOverlappingSets { sticker_id } => {
                write!(f, "(overlap:{sticker_id}) ")
            }
            InlineQueryData::ListSetStickersByDate { sticker_id } => {
                write!(f, "(dates:{sticker_id}) ")
            }
            InlineQueryData::ListAllTagsFromSet { sticker_id } => {
                write!(f, "(settags:{sticker_id}) ")
            }
            InlineQueryData::AddToUserSet {
                sticker_id,
                set_title,
            } => {
                write!(
                    f,
                    "(add:{sticker_id}) {}",
                    set_title.as_deref().unwrap_or_default()
                )
            }
            InlineQueryData::SearchStickers { emoji, tags } => {
                let tags = tags.join(" ");
                let emoji = emoji.into_iter().map(|e| e.to_string_with_variant()).join(" ");
                write!(f, "{emoji}")?;
                if emoji.len() > 0 {
                    write!(f, " ")?;
                }
                write!(f, "{tags}")
            }
            InlineQueryData::SearchTagsForBlacklist { tags } => {
                let tags = tags.join(" ");
                write!(f, "(blacklist) {tags}")
            }
            InlineQueryData::SearchTagsForStickerSet {
                set_name,
                operation: SetOperation::Tag,
                tags,
            } => {
                let tags = tags.into_iter().map(|t| t.join(" ")).join(", ");
                write!(f, "(se:{set_name}) {tags}")
            }
            InlineQueryData::SearchTagsForStickerSet {
                set_name,
                operation: SetOperation::Untag,
                tags,
            } => {
                let tags = tags.into_iter().map(|t| t.join(" ")).join(", ");
                write!(f, "(su:{set_name}) {tags}")
            }
            InlineQueryData::TagCreatorTagId {
                tag_id,
                kind: TagKind::Main,
            } => write!(f, "(tag) {tag_id}"),
            InlineQueryData::TagCreatorTagId {
                tag_id,
                kind: TagKind::Alias,
            } => write!(f, "(alias) {tag_id}"),
            InlineQueryData::SearchTagsForContinuousTagMode {
                operation: SetOperation::Tag,
                tags,
            } => {
                let tags = tags.into_iter().map(|t| t.join(" ")).join(", ");
                write!(f, "(cont) {tags}")
            }
            InlineQueryData::SearchTagsForContinuousTagMode {
                operation: SetOperation::Untag,
                tags,
            } => {
                let tags = tags.into_iter().map(|t| t.join(" ")).join(", ");
                write!(f, "(-cont) {tags}")
            }
            InlineQueryData::ListSimilarStickers {
                aspect: SimilarityAspect::Color,
                unique_id,
            } => write!(f, "(color:{unique_id}) "),
            // InlineQueryData::Similar {
            //     aspect: SimilarityAspect::Shape,
            //     unique_id,
            // } => write!(f, "(shape:{unique_id}) {tags}"),
            InlineQueryData::ListSimilarStickers {
                aspect: SimilarityAspect::Embedding,
                unique_id,
            } => write!(f, "(embed:{unique_id}) "),
            InlineQueryData::SearchByEmbedding { query } => write!(f, "(embed) {query}"),
            InlineQueryData::ListMostDuplicatedStickers => write!(f, "(dup) "),
            InlineQueryData::ListMostUsedEmojis => write!(f, "(emo) "),
            InlineQueryData::ListRecommendationModeRecommendations => write!(f, "(rec) "),
            InlineQueryData::SetsByUserId { user_id } => write!(f, "(usersets:{user_id}) "),
        }
    }
}

impl From<InlineQueryData> for String {
    fn from(value: InlineQueryData) -> Self {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn set_tag_operation() -> Result<()> {
        assert_eq!(
            InlineQueryData::try_from("(se:asdf) dragon uwu".to_string())?,
            InlineQueryData::SearchTagsForStickerSet {
                set_name: "asdf".to_string(),
                operation: SetOperation::Tag,
                tags: vec![vec!["dragon".to_string(), "uwu".to_string()]],
            }
        );
        Ok(())
    }

    #[test]
    fn stringify_query() {
        let query = InlineQueryData::empty_sticker_query("asdf");
        assert_eq!(query.to_string(), "(s:asdf) ");
    }

    #[test]
    fn stringify_tag_query() {
        let query = InlineQueryData::sticker_query(
            "asdf",
            vec![vec!["male".to_string(), "female".to_string()]],
        );
        assert_eq!(query.to_string(), "(s:asdf) male female");
    }

    #[test]
    fn stringify_tag_query_multiple() {
        let query = InlineQueryData::sticker_query(
            "asdf",
            vec![
                vec!["male".to_string(), "female".to_string()],
                vec!["asdf".to_string()],
            ],
        );
        assert_eq!(query.to_string(), "(s:asdf) male female, asdf");
    }

    #[test]
    fn parse_query() -> Result<(), UserError> {
        let query = InlineQueryData::try_from("male female".to_string())?;
        assert_eq!(
            query,
            InlineQueryData::search(vec!["male".to_string(), "female".to_string()])
        );
        Ok(())
    }

    #[test]
    fn parse_emoji_query() -> Result<(), UserError> {
        let query = InlineQueryData::try_from("🏳️‍🌈".to_string())?;
        let flag = Emoji::new_from_string_single("🏳️‍🌈");
        assert_eq!(query, InlineQueryData::search_emoji(vec![], vec![flag]));
        Ok(())
    }

    #[test]
    fn parse_blacklist_query() -> Result<(), UserError> {
        let query = InlineQueryData::try_from("(blacklist) attribution".to_string())?;
        assert_eq!(
            query,
            InlineQueryData::blacklist_query(vec!["attribution".to_string()])
        );
        Ok(())
    }
}
