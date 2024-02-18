use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::map;
use nom::combinator::opt;
use nom::multi::{many0, many1, separated_list0};
use nom::sequence::{preceded, terminated};
use nom::IResult;
use std::fmt::Display;

use crate::util::{parse_emoji, set_name_literal, sticker_id_literal, tag_literal, Emoji};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SetOperation {
    Tag,
    Untag,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InlineQueryData {
    pub tags: Vec<String>,
    pub mode: InlineQueryDataMode,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum InlineQueryDataMode {
    Sticker {
        unique_id: String,
    },
    Set {
        set_name: String,
        operation: SetOperation,
    },
    StickerSearch {
        emoji: Option<Emoji>,
    },
    Blacklist,
    ContinuousTagMode {
        operation: SetOperation,
    },
    Similar {
        unique_id: String,
        aspect: SimilarityAspect,
    },
    EmbeddingSearch,
}

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum SimilarityAspect {
    Color,
    Shape,
    Embedding,
}

impl InlineQueryData {
    #[must_use]
    pub fn empty_sticker_query(sticker_unique_id: impl Into<String>) -> Self {
        Self {
            mode: InlineQueryDataMode::Sticker {
                unique_id: sticker_unique_id.into(),
            },
            tags: vec![],
        }
    }

    #[must_use]
    pub fn similar(sticker_unique_id: impl Into<String>, aspect: SimilarityAspect) -> Self {
        Self {
            mode: InlineQueryDataMode::Similar {
                unique_id: sticker_unique_id.into(),
                aspect,
            },
            tags: vec![],
        }
    }

    #[must_use]
    pub fn embedding(tags: Vec<String>) -> Self {
        Self {
            mode: InlineQueryDataMode::EmbeddingSearch,
            tags,
        }
    }

    #[must_use]
    pub fn blacklist_query(tags: Vec<String>) -> Self {
        Self {
            mode: InlineQueryDataMode::Blacklist,
            tags,
        }
    }

    #[must_use]
    pub fn set_operation(
        set_name: impl Into<String>,
        tags: Vec<String>,
        operation: SetOperation,
    ) -> Self {
        Self {
            mode: InlineQueryDataMode::Set {
                set_name: set_name.into(),
                operation,
            },
            tags,
        }
    }

    #[must_use]
    pub fn continuous_tag_mode(tags: Vec<String>, operation: SetOperation) -> Self {
        Self {
            mode: InlineQueryDataMode::ContinuousTagMode { operation },
            tags,
        }
    }

    #[must_use]
    pub fn search(tags: Vec<String>) -> Self {
        Self {
            mode: InlineQueryDataMode::StickerSearch { emoji: None },
            tags,
        }
    }
}

fn parse_inline_query_data(input: &str) -> IResult<&str, InlineQueryData> {
    let (input, mut mode) = alt((
        map(
            terminated(preceded(tag("(s:"), sticker_id_literal), tag(")")),
            |unique_id| InlineQueryDataMode::Sticker {
                unique_id: unique_id.to_string(),
            },
        ),
        map(
            terminated(preceded(tag("(se:"), set_name_literal), tag(")")),
            |set_name| InlineQueryDataMode::Set {
                set_name: set_name.to_string(),
                operation: SetOperation::Tag,
            },
        ),
        map(
            terminated(preceded(tag("(su:"), set_name_literal), tag(")")),
            |set_name| InlineQueryDataMode::Set {
                set_name: set_name.to_string(),
                operation: SetOperation::Untag,
            },
        ),
        map(tag("(blacklist)"), |_| InlineQueryDataMode::Blacklist),
        map(tag("(embed)"), |_| InlineQueryDataMode::EmbeddingSearch),
        map(tag("(cont)"), |_| InlineQueryDataMode::ContinuousTagMode {
            operation: SetOperation::Tag,
        }),
        map(tag("(-cont)"), |_| InlineQueryDataMode::ContinuousTagMode {
            operation: SetOperation::Untag,
        }),
        map(
            terminated(preceded(tag("(color:"), sticker_id_literal), tag(")")),
            |unique_id| InlineQueryDataMode::Similar {
                aspect: SimilarityAspect::Color,
                unique_id: unique_id.to_string(),
            },
        ),
        map(
            terminated(preceded(tag("(shape:"), sticker_id_literal), tag(")")),
            |unique_id| InlineQueryDataMode::Similar {
                aspect: SimilarityAspect::Shape,
                unique_id: unique_id.to_string(),
            },
        ),
        map(
            terminated(preceded(tag("(embed:"), sticker_id_literal), tag(")")),
            |unique_id| InlineQueryDataMode::Similar {
                aspect: SimilarityAspect::Embedding,
                unique_id: unique_id.to_string(),
            },
        ),
        map(tag(""), |_| InlineQueryDataMode::StickerSearch {
            emoji: None,
        }),
    ))(input)?;
    let (input, _) = many0(tag(" "))(input)?;
    let (input, emoji) = opt(parse_emoji)(input)?;
    let (input, _) = many0(tag(" "))(input)?;
    if matches!(mode, InlineQueryDataMode::StickerSearch { .. }) {
        mode = InlineQueryDataMode::StickerSearch { emoji };
    }
    let (input, tags) = separated_list0(many1(tag(" ")), tag_literal)(input)?;
    Ok((
        input,
        InlineQueryData {
            mode,
            tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
        },
    ))
}

impl TryFrom<String> for InlineQueryData {
    type Error = String;

    fn try_from(input: String) -> Result<Self, Self::Error> {
        let (input, data) = parse_inline_query_data(&input)
            .map_err(|err| format!("invalid inline query: {err}"))?;
        if input.is_empty() {
            Ok(data)
        } else {
            Err(format!(
                "invalid inline query: input not consumed ({input})"
            ))
        }
    }
}

impl Display for InlineQueryData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tags = self.tags.join(" ");
        match &self.mode {
            InlineQueryDataMode::Sticker { unique_id } => write!(f, "(s:{unique_id}) {tags}"),
            InlineQueryDataMode::StickerSearch { emoji } => {
                if let Some(emoji) = emoji {
                    write!(f, "{emoji}")?;
                }
                write!(f, "{tags}")
            }
            InlineQueryDataMode::Blacklist => write!(f, "(blacklist) {tags}"),
            InlineQueryDataMode::Set {
                set_name,
                operation: SetOperation::Tag,
            } => write!(f, "(se:{set_name}) {tags}"),
            InlineQueryDataMode::Set {
                set_name,
                operation: SetOperation::Untag,
            } => write!(f, "(su:{set_name}) {tags}"),
            InlineQueryDataMode::ContinuousTagMode {
                operation: SetOperation::Tag,
            } => write!(f, "(cont) {tags}"),
            InlineQueryDataMode::ContinuousTagMode {
                operation: SetOperation::Untag,
            } => write!(f, "(-cont) {tags}"),
            InlineQueryDataMode::Similar {
                aspect: SimilarityAspect::Color,
                unique_id,
            } => write!(f, "(color:{unique_id}) {tags}"),
            InlineQueryDataMode::Similar {
                aspect: SimilarityAspect::Shape,
                unique_id,
            } => write!(f, "(shape:{unique_id}) {tags}"),
            InlineQueryDataMode::Similar {
                aspect: SimilarityAspect::Embedding,
                unique_id,
            } => write!(f, "(embed:{unique_id}) {tags}"),
            InlineQueryDataMode::EmbeddingSearch => write!(f, "(embed) {tags}"),
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

    fn sticker_query(sticker_unique_id: impl Into<String>, tags: Vec<String>) -> InlineQueryData {
        InlineQueryData {
            mode: InlineQueryDataMode::Sticker {
                unique_id: sticker_unique_id.into(),
            },
            tags,
        }
    }

    pub const fn tag_query(tags: Vec<String>, emoji: Option<Emoji>) -> InlineQueryData {
        InlineQueryData {
            mode: InlineQueryDataMode::StickerSearch { emoji },
            tags,
        }
    }

    #[test]
    fn stringify_query() {
        let query = InlineQueryData::empty_sticker_query("asdf");
        assert_eq!(query.to_string(), "(s:asdf) ");
    }

    #[test]
    fn stringify_tag_query() {
        let query = sticker_query("asdf", vec!["male".to_string(), "female".to_string()]);
        assert_eq!(query.to_string(), "(s:asdf) male female");
    }
    #[test]
    fn parse_query() -> Result<(), String> {
        let query = InlineQueryData::try_from("male female".to_string())?;
        assert_eq!(
            query,
            tag_query(vec!["male".to_string(), "female".to_string()], None)
        );
        Ok(())
    }

    #[test]
    fn parse_emoji_query() -> Result<(), String> {
        let query = InlineQueryData::try_from("🏳️‍🌈".to_string())?;
        let flag = Emoji::parse("🏳️‍🌈")
            .first()
            .expect("emoji to parse")
            .to_owned();
        assert_eq!(query, tag_query(vec![], Some(flag)));
        Ok(())
    }

    #[test]
    fn parse_blacklist_query() -> Result<(), String> {
        let query = InlineQueryData::try_from("(blacklist) attribution".to_string())?;
        assert_eq!(
            query,
            InlineQueryData::blacklist_query(vec!["attribution".to_string()])
        );
        Ok(())
    }
}
