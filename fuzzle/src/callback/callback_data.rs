use nom::bytes::complete::tag;

use nom::character::complete::u8;
use nom::combinator::{eof, fail, map};

use nom::sequence::{preceded, terminated, tuple};

use nom::IResult;

use nom::{branch::alt, character::complete::u64, combinator::opt};
use num_traits::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

use crate::database::StickerOrder;

use crate::tags::Category;
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FavoriteAction {
    Favorite,
    Unfavorite,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CallbackData {
    NoAction,
    RemoveLinkedChannel,
    RemoveLinkedUser,
    LinkSelf,
    CreateTag,
    Help,
    Start,
    Settings,
    Blacklist,
    FeatureOverview,
    ExitDialog,
    PopularTags,
    GeneralStats,
    PersonalStats,
    LatestSets,
    Info,

    RemoveBlacklistedTag(String),
    RemoveContinuousTag(String),
    RemoveAlias(String),
    UserInfo(u64),
    SetOrder(StickerOrder),
    SetCategory(Option<Category>),

    StickerSetPage {
        sticker_id: String,
    },
    DownloadSticker {
        sticker_id: String,
    },
    StickerExplorePage {
        sticker_id: String,
    },
    ToggleExampleSticker {
        sticker_id: String,
    },

    Sticker {
        sticker_id: String,
        operation: Option<TagOperation>,
    },
    FavoriteSticker {
        sticker_id: String,
        operation: FavoriteAction,
    },
    SetLock {
        sticker_id: String,
        lock: bool,
    },
    ToggleRecommendSticker {
        sticker_id: String,
        positive: bool,
    },

    ChangeSetStatus {
        set_name: String,
        banned: bool,
    },

    Merge {
        sticker_id_a: String,
        sticker_id_b: String,
        merge: bool,
    },
}

impl CallbackData {
    pub fn tag_sticker(sticker_id: impl Into<String>, tag: impl Into<String>) -> Self {
        Self::Sticker {
            sticker_id: sticker_id.into(),
            operation: Some(TagOperation::Tag(tag.into())),
        }
    }
    pub fn untag_sticker(sticker_id: impl Into<String>, tag: impl Into<String>) -> Self {
        Self::Sticker {
            sticker_id: sticker_id.into(),
            operation: Some(TagOperation::Untag(tag.into())),
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

    pub fn merge(
        sticker_id_a: impl Into<String>,
        sticker_id_b: impl Into<String>,
        merge: bool,
    ) -> Self {
        Self::Merge {
            merge,
            sticker_id_a: sticker_id_a.into(),
            sticker_id_b: sticker_id_b.into(),
        }
    }
}

fn parse_callback_data(input: &str) -> IResult<&str, CallbackData> {
    terminated(
        alt((
            parse_simple,
            parse_remove_blacklist_data,
            parse_remove_continuous_tag,
            parse_sticker_data,
            parse_remove_set_data,
            parse_user_info_data,
            parse_order_data,
            parse_set_category,
            parse_remove_alias,
            parse_lock_data,
            parse_recommend_sticker,
            parse_sticker_set_page,
            parse_download_sticker,
            parse_sticker_explore_page,
            parse_toggle_example_sticker,
            parse_favorite_sticker_data,
            parse_merge_data,
        )),
        eof,
    )(input)
}

fn parse_simple(input: &str) -> IResult<&str, CallbackData> {
    alt((
        map(tag("start"), |_| CallbackData::Start),
        map(tag("settings"), |_| CallbackData::Settings),
        map(tag("features"), |_| CallbackData::FeatureOverview),
        map(tag("exdia"), |_| CallbackData::ExitDialog),
        map(tag("pop"), |_| CallbackData::PopularTags),
        map(tag("gstats"), |_| CallbackData::GeneralStats),
        map(tag("pstats"), |_| CallbackData::PersonalStats),
        map(tag("lsets"), |_| CallbackData::LatestSets),
        map(tag("help"), |_| CallbackData::Help),
        map(tag("blacklist"), |_| CallbackData::Blacklist),
        map(tag("info"), |_| CallbackData::Info),
        map(tag("noaction"), |_| CallbackData::NoAction),
        map(tag("removeuser"), |_| CallbackData::RemoveLinkedUser),
        map(tag("linkself"), |_| CallbackData::LinkSelf),
        map(tag("createtag"), |_| CallbackData::CreateTag),
        map(tag("removechannel"), |_| CallbackData::RemoveLinkedChannel),
    ))(input)
}

fn parse_merge_data(input: &str) -> IResult<&str, CallbackData> {
    map(
        tuple((
            tag("merge;"),
            sticker_id_literal,
            tag(";"),
            sticker_id_literal,
            tag(";"),
            alt((map(tag("true"), |_| true), map(tag("false"), |_| false))),
        )),
        |(_, sticker_id_a, _, sticker_id_b, _, merge)| CallbackData::Merge {
            sticker_id_a: sticker_id_a.to_string(),
            sticker_id_b: sticker_id_b.to_string(),
            merge,
        },
    )(input)
}

fn parse_favorite_sticker_data(input: &str) -> IResult<&str, CallbackData> {
    map(
        tuple((
            tag("fav;"),
            sticker_id_literal,
            tag(";"),
            alt((
                map(tag("add"), |_| FavoriteAction::Favorite),
                map(tag("remove"), |_| FavoriteAction::Unfavorite),
            )),
        )),
        |(_, sticker_id, _, operation)| CallbackData::FavoriteSticker {
            sticker_id: sticker_id.to_string(),
            operation,
        },
    )(input)
}

fn parse_sticker_set_page(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("ssp;")(input)?;
    let (input, sticker_id) = sticker_id_literal(input)?;
    Ok((
        input,
        CallbackData::StickerSetPage {
            sticker_id: sticker_id.to_string(),
        },
    ))
}

fn parse_download_sticker(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("dls;")(input)?;
    let (input, sticker_id) = sticker_id_literal(input)?;
    Ok((
        input,
        CallbackData::DownloadSticker {
            sticker_id: sticker_id.to_string(),
        },
    ))
}

fn parse_sticker_explore_page(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("sep;")(input)?;
    let (input, sticker_id) = sticker_id_literal(input)?;
    Ok((
        input,
        CallbackData::StickerExplorePage {
            sticker_id: sticker_id.to_string(),
        },
    ))
}

fn parse_toggle_example_sticker(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("tex;")(input)?;
    let (input, sticker_id) = sticker_id_literal(input)?;
    Ok((
        input,
        CallbackData::ToggleExampleSticker {
            sticker_id: sticker_id.to_string(),
        },
    ))
}

fn parse_recommend_sticker(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("rec;")(input)?;
    let (input, sticker_id) = sticker_id_literal(input)?;
    let (input, _) = tag(";")(input)?;
    alt((
        map(tag("positive"), |_| CallbackData::ToggleRecommendSticker {
            positive: true,
            sticker_id: sticker_id.to_string(),
        }),
        map(tag("negative"), |_| CallbackData::ToggleRecommendSticker {
            positive: false,
            sticker_id: sticker_id.to_string(),
        }),
    ))(input)
}

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

fn parse_order_data(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("order;")(input)?;
    let order = serde_json::from_str(input);
    match order {
        Err(err) => fail(input),
        Ok(order) => Ok(("", CallbackData::SetOrder(order))),
    }
}

fn parse_set_category(input: &str) -> IResult<&str, CallbackData> {
    alt((
        map(preceded(tag("cat;"), u8), |cat| {
            CallbackData::SetCategory(Category::from_u8(cat))
        }),
        map(tag("cat;none"), |_| CallbackData::SetCategory(None)),
    ))(input)
}

fn parse_remove_alias(input: &str) -> IResult<&str, CallbackData> {
    map(preceded(tag("ras;"), tag_literal), |tag| {
        CallbackData::RemoveAlias(tag.to_string())
    })(input)
}

fn parse_user_info_data(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("userinfo")(input)?;
    let (input, _) = tag(";")(input)?;
    let (input, user_id) = u64(input)?;
    Ok((input, CallbackData::UserInfo(user_id)))
}

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

fn parse_remove_blacklist_data(input: &str) -> IResult<&str, CallbackData> {
    map(preceded(tag("removebl;"), tag_literal), |tag| {
        CallbackData::RemoveBlacklistedTag(tag.to_string())
    })(input)
}

fn parse_remove_continuous_tag(input: &str) -> IResult<&str, CallbackData> {
    map(preceded(tag("removec;"), tag_literal), |tag| {
        CallbackData::RemoveContinuousTag(tag.to_string())
    })(input)
}

fn parse_sticker_data(input: &str) -> IResult<&str, CallbackData> {
    let (input, _) = tag("s")(input)?;
    let (input, _) = tag(";")(input)?;
    let (input, sticker_id) = sticker_id_literal(input)?;
    let (input, _) = tag(";")(input)?;
    let (input, operation) = opt(parse_tag_operation)(input)?;
    Ok((
        input,
        CallbackData::Sticker {
            sticker_id: sticker_id.to_string(),
            operation,
        },
    ))
}

impl TryFrom<String> for CallbackData {
    type Error = anyhow::Error;

    fn try_from(input: String) -> Result<Self, Self::Error> {
        let (input, data) = parse_callback_data(&input)
            .map_err(|err| anyhow::anyhow!("invalid callback data: {}", err))?;
        Ok(data)
    }
}

impl Display for CallbackData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sticker {
                sticker_id,
                operation,
            } => {
                let operation = match operation {
                    Some(op) => op.to_string(),
                    None => String::new(),
                };
                write!(f, "s;{sticker_id};{operation}")
            }
            Self::Help => write!(f, "help"),
            Self::FeatureOverview => write!(f, "features"),
            Self::ExitDialog => write!(f, "exdia"),
            Self::PopularTags => write!(f, "pop"),
            Self::GeneralStats => write!(f, "gstats"),
            Self::PersonalStats => write!(f, "pstats"),
            Self::LatestSets => write!(f, "lsets"),
            Self::FavoriteSticker {
                operation,
                sticker_id,
            } => {
                let operation = match operation {
                    FavoriteAction::Favorite => "add",
                    FavoriteAction::Unfavorite => "remove",
                };
                write!(f, "fav;{sticker_id};{operation}")
            }
            Self::Settings => write!(f, "settings"),
            Self::Start => write!(f, "start"),
            Self::Blacklist => write!(f, "blacklist"),
            Self::NoAction => write!(f, "noaction"),
            Self::RemoveLinkedUser => write!(f, "removeuser"),
            Self::LinkSelf => write!(f, "linkself"),
            Self::CreateTag => write!(f, "createtag"),
            Self::RemoveLinkedChannel => write!(f, "removechannel"),
            Self::StickerSetPage { sticker_id } => write!(f, "ssp;{sticker_id}"),
            Self::DownloadSticker { sticker_id } => write!(f, "dls;{sticker_id}"),
            Self::StickerExplorePage { sticker_id } => write!(f, "sep;{sticker_id}"),
            Self::ToggleExampleSticker { sticker_id } => write!(f, "tex;{sticker_id}"),
            Self::RemoveBlacklistedTag(tag) => write!(f, "removebl;{tag}"),
            Self::RemoveContinuousTag(tag) => write!(f, "removec;{tag}"),
            Self::RemoveAlias(tag) => write!(f, "ras;{tag}"),
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
            Self::SetCategory(category) => {
                let category = match category {
                    Some(category) => category.to_u8().unwrap_or_default().to_string(),
                    None => "none".to_string(),
                };
                write!(f, "cat;{category}")
            }
            Self::SetLock { lock, sticker_id } => {
                let lock = if *lock { "lock" } else { "unlock" };
                write!(f, "lock;{sticker_id};{lock}")
            }
            Self::ToggleRecommendSticker {
                positive,
                sticker_id,
            } => {
                let positive = if *positive { "positive" } else { "negative" };
                write!(f, "rec;{sticker_id};{positive}")
            }
            Self::Merge {
                merge,
                sticker_id_a,
                sticker_id_b,
            } => {
                let merge = if *merge { "true" } else { "false" };
                write!(f, "merge;{sticker_id_a};{sticker_id_b};{merge}")
            }
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
                sticker_id: "5uh33fj84".to_string(),
                operation: Some(TagOperation::Tag("male".to_string())),
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
                sticker_id: "5uh33fj84".to_string(),
                operation: Some(TagOperation::Untag("male".to_string())),
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
