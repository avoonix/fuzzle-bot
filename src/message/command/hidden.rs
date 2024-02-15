use crate::bot::{Bot, BotExt, UserMeta};
use crate::callback::TagOperation;
use crate::database::Database;
use crate::inline::SetOperation;
use crate::message::Keyboard;
use crate::tags::{suggest_tags, TagManager};
use crate::text::{Markdown, Text};
use crate::worker::WorkerPool;
use anyhow::Result;
use itertools::Itertools;
use teloxide::types::{InputFile, ReplyMarkup};

use std::sync::Arc;
use teloxide::{prelude::*, utils::command::BotCommands};

use super::unescape_sticker_unique_id_from_command;

#[derive(BotCommands, Debug)]
#[command(rename_rule = "lowercase", description = "Hidden commands")]
pub enum HiddenCommand {
    #[command(
        description = "tag a sticker (do not use manually)",
        parse_with = "split"
    )]
    TagSticker {
        sticker_unique_id: String,
        tag: String,
    },
    #[command(description = "find sets containing a sticker (do not use manually)")]
    FindSets { sticker_unique_id: String },
    #[command(description = "add a tag to the blacklist (do not use manually)")]
    BlacklistTag { tag: String },
    #[command(
        description = "tags all stickers in a set (do not use manually)",
        parse_with = "split"
    )]
    TagSet { set_name: String, tag: String },
    #[command(
        description = "untags all stickers in a set (do not use manually)",
        parse_with = "split"
    )]
    UntagSet { set_name: String, tag: String },
    #[command(description = "tags stickers continuously upon reply (do not use manually)")]
    TagContinuous { tag: String },
    #[command(description = "untags stickers continuously upon reply (do not use manually)")]
    UntagContinuous { tag: String },
    #[command(description = "cancel a pending operation (continuous tag mode)")]
    Cancel,
    #[command(
        description = "display menu for adding or removing tags from sets (do not use manually)"
    )]
    SetOps { sticker_unique_id: String },
}

impl HiddenCommand {
    pub async fn execute(
        self,
        bot: Bot,
        msg: Message,
        tag_manager: Arc<TagManager>,
        worker: WorkerPool,
        database: Database,
        user: UserMeta,
    ) -> Result<()> {
        match self {
            Self::TagSticker {
                sticker_unique_id,
                tag,
            } => {
                if !user.can_tag_stickers() {
                    return Err(anyhow::anyhow!(
                        "user is not permitted to tag stickers (hidden command)"
                    ))?;
                }
                let sticker = database.get_sticker(sticker_unique_id.clone()).await?;
                if let Some(sticker) = sticker {
                    if let Some(implications) = tag_manager.get_implications(&tag) {
                        let tags = implications
                            .into_iter()
                            .chain(std::iter::once(tag.clone()))
                            .collect_vec();
                        database
                            .tag_sticker(sticker_unique_id.clone(), tags, Some(user.id().0))
                            .await?;
                    }
                    let tags = database.get_sticker_tags(sticker_unique_id.clone()).await?;
                    let suggested_tags = suggest_tags(
                        &sticker_unique_id,
                        bot.clone(),
                        tag_manager,
                        database.clone(),
                    )
                    .await?;
                    let set_name = database.get_set_name(sticker_unique_id.clone()).await?;
                    bot.send_sticker(msg.chat.id, InputFile::file_id(sticker.file_id)) // TODO: fetch
                        // file id
                        // from
                        // database
                        .reply_markup(Keyboard::make_tag_keyboard(
                            &tags,
                            &sticker_unique_id,
                            &suggested_tags,
                            set_name,
                        ))
                        .await?;
                } else {
                    bot.send_markdown(
                        msg.chat.id,
                        Markdown::escaped(format!(
                            "Could not find sticker `{}`",
                            sticker_unique_id.clone()
                        )),
                    )
                    .await?;
                }
            }
            Self::FindSets { sticker_unique_id } => {
                let sticker_unique_id = unescape_sticker_unique_id_from_command(&sticker_unique_id);
                let sets = database
                    .get_sets_containing_sticker(sticker_unique_id.clone())
                    .await?;
                let messages = Text::get_find_set_messages(sets, &sticker_unique_id);
                for (position, message) in messages.into_iter().with_position() {
                    match position {
                        itertools::Position::First | itertools::Position::Middle => {
                            bot.send_markdown(msg.chat.id, message).await?
                        }
                        itertools::Position::Last | itertools::Position::Only => {
                            bot.send_markdown(msg.chat.id, message)
                                .reply_markup(Keyboard::similarity(&sticker_unique_id))
                                .await?
                        }
                    };
                }
            }
            Self::SetOps { sticker_unique_id } => {
                let sticker_unique_id = unescape_sticker_unique_id_from_command(&sticker_unique_id);
                let Some(set) = database.get_set(sticker_unique_id).await? else {
                    bot.send_markdown(msg.chat.id, Markdown::escaped("unknown set"))
                        .await?;
                    return Ok(());
                };
                bot.send_markdown(
                    msg.chat.id,
                    Text::get_set_operations_text(&set.id, set.title.as_ref().unwrap_or(&set.id)),
                )
                .reply_markup(Keyboard::make_set_keyboard(set.id))
                .await?;
            }
            Self::BlacklistTag { tag } => {
                database.add_tag_to_blacklist(user.id().0, tag).await?;
                let blacklist = database.get_blacklist(user.id().0).await?;
                bot.send_markdown(msg.chat.id, Text::get_blacklist_text())
                    .reply_markup(Keyboard::make_blacklist_keyboard(&blacklist))
                    .await?;
            }
            Self::TagSet { set_name, tag } => {
                set_tag_operation(
                    tag,
                    set_name,
                    SetOperation::Tag,
                    user,
                    worker,
                    tag_manager,
                    bot,
                    database,
                    msg,
                )
                .await?;
            }
            Self::UntagSet { set_name, tag } => {
                set_tag_operation(
                    tag,
                    set_name,
                    SetOperation::Untag,
                    user,
                    worker,
                    tag_manager,
                    bot,
                    database,
                    msg,
                )
                .await?;
            }
            Self::TagContinuous { tag } => {
                bot.send_markdown(
                    msg.chat.id,
                    Text::get_continuous_tag_mode_text(TagOperation::Tag(tag), "Start".to_string()),
                )
                .reply_markup(ReplyMarkup::ForceReply(
                    teloxide::types::ForceReply::new()
                        .input_field_placeholder(Some("Reply with a sticker".to_string())),
                ))
                .await?;
            }
            Self::UntagContinuous { tag } => {
                bot.send_markdown(
                    msg.chat.id,
                    Text::get_continuous_tag_mode_text(
                        TagOperation::Untag(tag),
                        "Start".to_string(),
                    ),
                )
                .reply_markup(ReplyMarkup::ForceReply(
                    teloxide::types::ForceReply::new()
                        .input_field_placeholder(Some("Reply with a sticker".to_string())),
                ))
                .await?;
            }
            Self::Cancel => {
                let tag = msg
                    .reply_to_message()
                    .and_then(Text::parse_continuous_tag_mode_message);
                bot.send_markdown(
                    msg.chat.id,
                    Markdown::escaped(if tag.is_some() {
                        "Cancelled Continuous Tag Mode"
                    } else {
                        "This message has to be sent as reply to my message"
                    }),
                )
                .await?;
            }
        }

        Ok(())
    }
}

async fn set_tag_operation(
    tag: String,
    set_name: String,
    operation: SetOperation,
    user: UserMeta,
    worker: WorkerPool,
    tag_manager: Arc<TagManager>,
    bot: Bot,
    database: Database,
    msg: Message,
) -> Result<()> {
    if !user.can_tag_sets() {
        return Err(anyhow::anyhow!(
            "user is not permitted to tag sets (hidden command)"
        ))?;
    }
    let message = match operation {
        SetOperation::Tag => {
            if let Some(implications) = tag_manager.get_implications(&tag) {
                let tags = implications
                    .into_iter()
                    .chain(std::iter::once(tag.clone()))
                    .collect_vec();
                let taggings_changed = database
                    .tag_all_stickers_in_set(set_name.clone(), tags, user.id().0)
                    .await?;
                Markdown::escaped(format!("Changed {taggings_changed} taggings (including implied tags) in set `{set_name}` with tag `{tag}`"))
            } else {
                Markdown::escaped("No tags changed".to_string())
            }
        }
        SetOperation::Untag => {
            let taggings_changed = database
                .untag_all_stickers_in_set(set_name.clone(), tag.clone(), user.id().0)
                .await?;
            Markdown::escaped(format!(
                "Changed {taggings_changed} taggings in set `{set_name}` (removed tag `{tag}`)"
            ))
        }
    };
    bot.send_markdown(
        msg.chat.id,
        message, // TODO: do not escape the `` -> tags should be inline code
    )
    .reply_to_message_id(msg.id)
    .allow_sending_without_reply(true)
    .reply_markup(Keyboard::make_set_keyboard(set_name))
    .await?;
    Ok(())
}
