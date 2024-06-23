use std::str::FromStr;
use std::sync::Arc;

use crate::bot::{BotError, BotExt, RequestContext, UserError};
use crate::callback::{exit_mode, TagOperation};

use crate::database::DialogState;
use crate::inline::SetOperation;
use crate::message::Keyboard;
use crate::tags::suggest_tags;
use crate::text::{Markdown, Text};
use crate::util::Required;
use itertools::Itertools;
use teloxide::types::{
    ButtonRequest, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, KeyboardButton,
    KeyboardButtonRequestChat, KeyboardButtonRequestUser, KeyboardMarkup, ReplyMarkup,
};

use teloxide::{prelude::*, utils::command::BotCommands};

use super::{send_sticker_with_tag_input, unescape_sticker_unique_id_from_command};

#[derive(Debug)]
pub struct TagList(Vec<String>);

impl FromStr for TagList {
    type Err = BotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.split(",").map(|s| s.to_string()).collect_vec()))
    }
}

#[derive(BotCommands, Debug)]
#[command(rename_rule = "lowercase", description = "Hidden commands")]
pub enum HiddenCommand {
    #[command(
        description = "tag a sticker (do not use manually)",
        parse_with = "split"
    )]
    TagSticker {
        sticker_unique_id: String,
        tag: TagList,
    },
    #[command(description = "add a tag to the blacklist (do not use manually)")]
    BlacklistTag { tag: String },
    #[command(
        description = "tags all stickers in a set (do not use manually)",
        parse_with = "split"
    )]
    TagSet { set_name: String, tag: TagList },
    #[command(
        description = "untags all stickers in a set (do not use manually)",
        parse_with = "split"
    )]
    UntagSet { set_name: String, tag: TagList },
    #[command(description = "tags stickers continuously upon reply (do not use manually)")]
    TagContinuous { tag: TagList },
    #[command(description = "untags stickers continuously upon reply (do not use manually)")]
    UntagContinuous { tag: TagList },
    #[command(description = "cancel a pending operation (continuous tag mode)", aliases = ["x", "exit"], hide_aliases)]
    Cancel,
    #[command(description = "get a random sticker")]
    Random,
}

impl HiddenCommand {
    #[tracing::instrument(skip(self, msg, request_context))]
    pub async fn execute(self, msg: Message, request_context: RequestContext) -> Result<(), BotError> {
        match self {
            Self::Random => {
                let request_context = if request_context.is_continuous_tag_state() {
                    exit_mode(request_context.clone(), false).await?
                } else {
                    request_context
                };
                let sticker = request_context.database.get_random_sticker_to_tag().await?.required()?;
                send_sticker_with_tag_input(
                    sticker,
                    request_context.clone(),
                    msg.chat.id,
                    msg.id,
                )
                .await?;
            }
            Self::TagSticker {
                sticker_unique_id,
                tag,
            } => {
                if !request_context.can_tag_stickers() {
                    return Err(anyhow::anyhow!(
                        "user is not permitted to tag stickers (hidden command)"
                    ))?;
                }
                let sticker = request_context.database.get_sticker_by_id(&sticker_unique_id).await?;
                if let Some(sticker) = sticker {
                    if let Some(tags) = get_tag_implications_including_self(&tag.0, request_context.clone()) {
                        request_context.database
                            .tag_file(&sticker.sticker_file_id, &tags, Some(request_context.user.id))
                            .await?;
                        request_context.tagging_worker.maybe_recompute().await?;
                    }
                    let tags = request_context.database.get_sticker_tags(&sticker_unique_id).await?;
                    let suggested_tags = suggest_tags(
                        &sticker_unique_id,
                        request_context.bot.clone(),
                        request_context.tag_manager.clone(),
                        request_context.database.clone(),
                        request_context.tagging_worker.clone(),
                        request_context.vector_db.clone(),
                        // request_context.tag_worker.clone(),
                    )
                    .await?;
                    let is_locked = request_context.database
                        .get_sticker_file_by_sticker_id(&sticker_unique_id)
                        .await?
                        .map_or(false, |file| file.tags_locked_by_user_id.is_some());
                    request_context.bot.send_sticker(msg.chat.id, InputFile::file_id(sticker.telegram_file_identifier)) // TODO: fetch
                        // file id
                        // from
                        // database
                        .reply_markup(Keyboard::tagging(
                            &tags,
                            &sticker_unique_id,
                            &suggested_tags,
                            is_locked,
                            request_context.is_continuous_tag_state(),
                        ))
                        .await?;
                } else {
                    request_context.bot.send_markdown(
                        msg.chat.id,
                        Markdown::escaped(format!(
                            "Could not find sticker `{}`",
                            sticker_unique_id.clone()
                        )),
                    )
                    .await?;
                }
            }
            Self::BlacklistTag { tag } => {
                let blacklist = request_context
                    .user
                    .blacklist
                    .clone()
                    .into_inner()
                    .into_iter()
                    .filter(|t| t != &tag)
                    .chain(std::iter::once(tag.clone()))
                    .collect_vec();
                request_context.database
                    .update_user_blacklist(
                        request_context.user.id,
                        blacklist.clone().into(),
                    )
                    .await?;
                request_context.bot.send_markdown(msg.chat.id, Text::blacklist())
                    .reply_markup(Keyboard::blacklist(&blacklist))
                    .await?;
            }
            Self::TagSet { set_name, tag } => {
                set_tag_operation(tag.0, set_name, SetOperation::Tag, msg, request_context).await?;
            }
            Self::UntagSet { set_name, tag } => {
                set_tag_operation(tag.0, set_name, SetOperation::Untag, msg, request_context).await?;
            }
            Self::TagContinuous { tag } => {
                modify_continuous_tag(tag.0, vec![], request_context, msg).await?;
            }
            Self::UntagContinuous { tag } => {
                modify_continuous_tag(vec![], tag.0, request_context, msg).await?;
            }
            Self::Cancel => {
                exit_mode(request_context.clone(), true).await?;
            }
        }

        Ok(())
    }
}

fn get_tag_implications_including_self(tags: &[String], request_context: RequestContext) -> Option<Vec<String>> {
    let tags = tags.into_iter().flat_map(|tag| request_context.tag_manager.get_implications_including_self(tag)).flatten().sorted().dedup().collect_vec();
    if tags.is_empty() {
        None
    } else {
        Some(tags)
    }
}

fn add_new_tags_to_continuous_tag(current: Vec<String>, added: Vec<String>, fixed_tags_from_other_section: &[String]) -> Vec<String> {
    current
        .into_iter()
        .chain(added)
        .filter(|tag| !fixed_tags_from_other_section.contains(tag))
        .sorted()
        .dedup()
        .collect_vec()
}

async fn modify_continuous_tag(new_add_tags: Vec<String>, new_remove_tags: Vec<String>, request_context: RequestContext, msg: Message) -> Result<(), BotError> {
    let new_add_tags = new_add_tags.into_iter().filter_map(|tag| request_context.tag_manager.get_implications_including_self(&tag)).flatten().collect_vec();
    let new_remove_tags = new_remove_tags.into_iter().filter_map(|tag| request_context.tag_manager.get_implications_including_self(&tag)).flatten().collect_vec();
    let (state_changed, (add_tags, remove_tags)) = match request_context.dialog_state() {
        DialogState::ContinuousTag {
            add_tags,
            remove_tags,
        } => (
            false, (add_tags, remove_tags)
        ),
        _ => ( true, ( vec![], vec![])),
    };

    let remove_tags = add_new_tags_to_continuous_tag(remove_tags, new_remove_tags, &new_add_tags);
    let add_tags = add_new_tags_to_continuous_tag(add_tags, new_add_tags, &remove_tags);

    let new_dialog_state = DialogState::ContinuousTag { add_tags: add_tags.clone(), remove_tags: remove_tags.clone() };
    request_context.database
        .update_dialog_state(request_context.user.id, &new_dialog_state)
        .await?;
    if state_changed {
        let message = request_context.bot.send_markdown(
                    msg.chat.id,
                    Markdown::escaped("You are now in continuous tag mode. Send stickers to tag them with all selected tags")
                )
                .reply_markup(
                    ReplyMarkup::Keyboard(KeyboardMarkup::new(vec![
                        vec![
                           KeyboardButton::new("/continuoustagmode"),
                        ],
                        vec![
                           KeyboardButton::new("/cancel"),
                        ]
                    ])
                    .resize_keyboard()
                    .persistent()
                .input_field_placeholder("Continuous Tag Mode"))
            )
                .await?;
        // request_context.bot.pin_chat_message(msg.chat.id, message.id).await?; // TODO: pin does not work until the user leaves and opens the chat??
    }
    let message = request_context.bot
        .send_markdown(
            msg.chat.id,
            Text::get_continuous_tag_mode_text(add_tags.as_slice(), remove_tags.as_slice()),
        )
        .reply_markup(Keyboard::make_continuous_tag_keyboard(true, add_tags.as_slice(), remove_tags.as_slice()))
        .await?;
    Ok(())
}

async fn set_tag_operation(
    tags: Vec<String>,
    set_name: String,
    operation: SetOperation,
    msg: Message,
    request_context: RequestContext,
) -> Result<(), BotError> {
    if !request_context.can_tag_sets() {
        return Err(UserError::NoPermissionForAction("tag set".to_string()).into());
    }
    let message = match operation {
        SetOperation::Tag => {
            if let Some(tags) = get_tag_implications_including_self(&tags, request_context.clone()) {
                let taggings_changed = request_context
                    .database
                    .tag_all_files_in_set(
                        &set_name,
                        tags.as_ref(),
                        request_context.user.id,
                    )
                    .await?;
                request_context.tagging_worker.maybe_recompute().await?;
                Text::tagged_set(&set_name, &tags, taggings_changed)
            } else {
                Markdown::escaped("No tags changed".to_string())
            }
        }
        SetOperation::Untag => {
            let taggings_changed = request_context
                .database
                .untag_all_files_in_set(
                    &set_name,
                    &tags,
                    request_context.user.id,
                )
                .await?;
            Text::untagged_set(&set_name, &tags, taggings_changed)
        }
    };
    request_context
        .bot
        .send_markdown(
            msg.chat.id,
            message, // TODO: do not escape the `` -> tags should be inline code
        )
        .reply_to_message_id(msg.id)
        .allow_sending_without_reply(true)
        .reply_markup(Keyboard::make_set_keyboard(set_name))
        .await?;
    Ok(())
}
