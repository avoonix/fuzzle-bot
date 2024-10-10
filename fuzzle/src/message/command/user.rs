use crate::bot::{BotError, BotExt, RequestContext};

use crate::callback::exit_mode;
use crate::database::{DialogState, Sticker, TagCreator};
use crate::message::Keyboard;
use crate::tags::suggest_tags;
use crate::text::{Markdown, Text};

use teloxide::types::{
    BotCommand, InputFile, KeyboardButton, KeyboardMarkup, LinkPreviewOptions, MessageId, ReplyMarkup
};
use tracing::warn;

use teloxide::{prelude::*, utils::command::BotCommands};

use super::privacy::PrivacyPolicy;
use super::StartParameter;

#[derive(BotCommands, Debug, Clone, Copy)]
#[command(rename_rule = "lowercase", description = "Supported commands")]
pub enum RegularCommand {
    #[command(description = "show settings")]
    Settings,

    #[command(description = "get a random sticker that needs tagging")]
    TagMe,

    #[command(description = "tag multiple stickers with the same tag")]
    ContinuousTagMode,

    #[command(description = "get sticker recommendations")]
    StickerRecommenderMode,

    #[command(description = "add your own character or artist tags")]
    TagCreator,

    #[command(description = "general statistics")]
    Stats,

    #[command(description = "clear recently used stickers")]
    ClearRecentlyUsed,

    #[command(description = "show welcome message")]
    Start { start_parameter: StartParameter },

    #[command(description = "display help text")]
    Help,

    #[command(description = "show privacy information")]
    Privacy,
}

impl RegularCommand {
    #[must_use]
    pub fn list_visible() -> Vec<BotCommand> {
        Self::bot_commands()
    }

    #[tracing::instrument(skip(self, msg, request_context))]
    pub async fn execute(
        self,
        msg: Message,
        request_context: RequestContext,
    ) -> Result<(), BotError> {
        match self {
            Self::Privacy => {
                request_context
                    .bot
                    .send_markdown(msg.chat.id, Text::privacy(PrivacyPolicy::Introduction))
                    .reply_markup(Keyboard::privacy(PrivacyPolicy::Introduction))
                    .await?;
            }
            Self::Help => {
                request_context
                    .bot
                    .send_markdown(msg.chat.id, Text::get_help_text(request_context.is_admin()))
                    .reply_markup(Keyboard::make_help_keyboard())
                    .await?;
            }
            Self::Start { start_parameter } => {
                let request_context = exit_mode(request_context.clone(), false).await?;
                match start_parameter {
                    StartParameter::Blacklist => {
                        request_context
                            .bot
                            .send_markdown(msg.chat.id, Text::blacklist())
                            .reply_markup(Keyboard::blacklist(&request_context.user.blacklist))
                            .await?;
                    }
                    StartParameter::Regular | StartParameter::Greeting => {
                        if let Some(greeting_sticker_id) =
                            request_context.config.greeting_sticker_id.clone()
                        {
                            let sticker = request_context
                                .database
                                .get_sticker_by_id(&greeting_sticker_id)
                                .await?;
                            if let Some(sticker) = sticker {
                                request_context
                                    .bot
                                    .send_sticker(
                                        msg.chat.id,
                                        InputFile::file_id(sticker.telegram_file_identifier),
                                    )
                                    .disable_notification(true)
                                    .await?;
                            } else {
                                warn!("greeting sticker is not in database");
                            }
                        } else {
                            warn!("greeting sticker id is not set");
                        }
                        request_context
                            .bot
                            .send_markdown(msg.chat.id, Text::get_start_text())
                            .reply_to_message_id(msg.id)
                            .allow_sending_without_reply(true)
                            .await?;
                        request_context
                            .bot
                            .send_markdown(msg.chat.id, Text::get_main_text())
                            .reply_markup(Keyboard::make_main_keyboard())
                            .disable_notification(true)
                            .await?;
                    }
                    StartParameter::Help => {
                        request_context
                            .bot
                            .send_markdown(
                                msg.chat.id,
                                Text::get_help_text(request_context.is_admin()),
                            )
                            .reply_markup(Keyboard::make_help_keyboard())
                            .await?;
                    }
                }
            }
            Self::Settings => {
                request_context
                    .bot
                    .send_markdown(
                        msg.chat.id,
                        Text::get_settings_text(
                            &request_context.user.settings.clone().unwrap_or_default(),
                        ),
                    )
                    .reply_markup(Keyboard::make_settings_keyboard(
                        &request_context.user.settings.clone().unwrap_or_default(),
                    ))
                    .await?;
            }
            Self::Stats => {
                let stats = request_context.database.get_stats().await?;
                request_context
                    .bot
                    .send_markdown(msg.chat.id, Text::general_stats(stats))
                    .link_preview_options(LinkPreviewOptions::new()
                    .is_disabled(true)
                )
                    .reply_markup(Keyboard::general_stats())
                    .await?;
            }
            Self::TagMe => {
                let request_context = exit_mode(request_context.clone(), false).await?;
                let sticker = request_context.database.get_random_sticker_to_tag().await?;
                if let Some(sticker) = sticker {
                    send_sticker_with_tag_input(
                        sticker,
                        request_context.clone(),
                        msg.chat.id,
                        msg.id,
                    )
                    .await?;
                } else {
                    request_context
                        .bot
                        .send_markdown(msg.chat.id, Markdown::escaped("No stickers to tag!"))
                        .await?;
                }
            }
            Self::ContinuousTagMode => match request_context.dialog_state() {
                DialogState::Normal
                | DialogState::StickerRecommender { .. }
                | DialogState::TagCreator { .. } => {
                    request_context
                        .bot
                        .send_markdown(
                            msg.chat.id,
                            Markdown::escaped("Choose a tag to apply to multiple stickers"),
                        )
                        .reply_markup(Keyboard::make_continuous_tag_keyboard(false, &[], &[]))
                        .await?;
                }
                DialogState::ContinuousTag (continuous_tag) => {
                    request_context
                        .bot
                        .send_markdown(
                            msg.chat.id,
                            Text::get_continuous_tag_mode_text(
                                continuous_tag.add_tags.as_slice(),
                                continuous_tag.remove_tags.as_slice(),
                            ),
                        )
                        .reply_markup(Keyboard::make_continuous_tag_keyboard(
                            true,
                            continuous_tag.add_tags.as_slice(),
                            continuous_tag.remove_tags.as_slice(),
                        ))
                        .await?;
                }
            },
            Self::TagCreator => match request_context.dialog_state() {
                DialogState::Normal
                | DialogState::StickerRecommender { .. }
                | DialogState::ContinuousTag { .. } => {
                    request_context
                        .bot
                        .send_markdown(msg.chat.id, Markdown::escaped("Give your new tag a name"))
                        .reply_markup(Keyboard::tag_creator_initial())
                        .await?;
                }
                DialogState::TagCreator(tag_creator) => {
                    request_context
                        .bot
                        .send_markdown(
                            msg.chat.id,
                            Markdown::escaped(format!("Creating tag: {}\nExamples: {}", &tag_creator.tag_id, &tag_creator.example_sticker_id.len())),
                        )
                        .reply_markup(Keyboard::tag_creator(&tag_creator))
                        .await?;
                }
            },
            Self::StickerRecommenderMode => {
                let (positive_sticker_id, negative_sticker_id) =
                    match request_context.dialog_state() {
                        DialogState::Normal
                        | DialogState::ContinuousTag { .. }
                        | DialogState::TagCreator { .. } => {
                            request_context
                                .database
                                .update_dialog_state(
                                    request_context.user.id,
                                    &DialogState::StickerRecommender {
                                        positive_sticker_id: vec![],
                                        negative_sticker_id: vec![],
                                    },
                                )
                                .await?;
                            (vec![], vec![])
                        }
                        DialogState::StickerRecommender {
                            negative_sticker_id,
                            positive_sticker_id,
                        } => (positive_sticker_id, negative_sticker_id),
                    };

                request_context
                    .bot
                    .send_markdown(
                        msg.chat.id,
                        Text::sticker_recommender_text(
                            positive_sticker_id.len(),
                            negative_sticker_id.len(),
                        ),
                    )
                    .reply_markup(ReplyMarkup::Keyboard(
                        KeyboardMarkup::new(vec![
                            // TODO: move to keyboard struct
                            vec![KeyboardButton::new("/stickerrecommendermode")],
                            vec![KeyboardButton::new("/random"), KeyboardButton::new("/autosuggest")],
                            vec![KeyboardButton::new("/cancel")],
                        ])
                        .resize_keyboard()
                        .persistent()
                        .input_field_placeholder("Sticker Recommender Mode"),
                    ))
                    .await?;
            }
            Self::ClearRecentlyUsed => {
                request_context
                    .database
                    .clear_recently_used_stickers(request_context.user.id)
                    .await?;
                request_context
                    .bot
                    .send_markdown(
                        msg.chat.id,
                        Markdown::escaped("Cleared recently used stickers"),
                    )
                    .reply_to_message_id(msg.id)
                    .allow_sending_without_reply(true)
                    .await?;
            } // TODO: this kind of reply markup can be used to have users check the correctness of tags
              // quickly
              // .reply_markup(ReplyMarkup::keyboard(vec![
              //                                     vec![
              //     KeyboardButton::new("correct"),
              //     KeyboardButton::new("incorrect"),
              //                                     ],
              // ]))
        }

        Ok(())
    }
}

pub async fn send_sticker_with_tag_input(
    sticker: Sticker,
    request_context: RequestContext,
    message_chat_id: ChatId,
    message_id: MessageId,
) -> Result<(), BotError> {
    request_context
        .bot
        .send_sticker(
            message_chat_id,
            InputFile::file_id(sticker.telegram_file_identifier),
        )
        .reply_markup(match request_context.dialog_state() {
            DialogState::Normal | DialogState::ContinuousTag { .. } => {
                let tags = request_context
                    .database
                    .get_sticker_tags(&sticker.id)
                    .await?;
                let suggested_tags = suggest_tags(
                    &sticker.id,
                    request_context.bot.clone(),
                    request_context.tag_manager.clone(),
                    request_context.database.clone(),
                    request_context.tagging_worker.clone(),
                    request_context.vector_db.clone(),
                    // request_context.tag_worker.clone(),
                )
                .await?;
                let is_locked = request_context
                    .database
                    .get_sticker_file_by_sticker_id(&sticker.id)
                    .await?
                    .map_or(false, |file| file.tags_locked_by_user_id.is_some());
                Keyboard::tagging(
                    &tags,
                    &sticker.id,
                    &suggested_tags,
                    is_locked,
                    request_context.is_continuous_tag_state(),
        request_context.tag_manager.clone(),
    ).await?
            }
            DialogState::StickerRecommender {
                positive_sticker_id,
                negative_sticker_id,
            } => {
                let sticker_user = request_context
                    .database
                    .get_sticker_user(&sticker.id, request_context.user.id)
                    .await?;
                let is_favorited = sticker_user.map_or(false, |su| su.is_favorite);

                Keyboard::recommender(
                    &sticker.id,
                    &positive_sticker_id,
                    &negative_sticker_id,
                    is_favorited,
                )
            }
            DialogState::TagCreator(tag_creator) => {
                Keyboard::tag_creator(&tag_creator)
            }
        })
        .reply_to_message_id(message_id)
        .allow_sending_without_reply(true)
        .await?;
    Ok(())
}
