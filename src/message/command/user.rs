use crate::bot::{Bot, BotExt, UserMeta};
use crate::database::Database;
use crate::message::Keyboard;
use crate::tags::{suggest_tags, TagManager};
use crate::text::{Markdown, Text};
use anyhow::Result;
use teloxide::types::{BotCommand, InputFile};

use std::sync::Arc;
use teloxide::{prelude::*, utils::command::BotCommands};

use super::StartParameter;

#[derive(BotCommands, Debug, Clone, Copy)]
#[command(rename_rule = "lowercase", description = "Supported commands")]
pub enum RegularCommand {
    #[command(description = "display help text")]
    Help,
    #[command(description = "show welcome message")]
    Start { start_parameter: StartParameter },
    #[command(description = "show settings")]
    Settings,
    #[command(description = "list popular tags")]
    PopularTags,
    #[command(description = "general statistics")]
    Stats,
    #[command(description = "get a random sticker that needs tagging")]
    TagMe,
    #[command(description = "tag multiple stickers with the same tag")]
    ContinuousTagMode,
}

impl RegularCommand {
    #[must_use]
    pub fn list_visible() -> Vec<BotCommand> {
        Self::bot_commands()
    }

    pub async fn execute(
        self,
        bot: Bot,
        msg: Message,
        tag_manager: Arc<TagManager>,
        database: Database,
        user: UserMeta,
    ) -> Result<()> {
        match self {
            Self::Help => {
                bot.send_markdown(msg.chat.id, Text::get_help_text(user.is_admin))
                    .reply_markup(Keyboard::make_help_keyboard())
                    .await?;
            }
            Self::Start { start_parameter } => match start_parameter {
                StartParameter::Blacklist => {
                    let blacklist = database.get_blacklist(user.id().0).await?;
                    bot.send_markdown(msg.chat.id, Text::get_blacklist_text())
                        .reply_markup(Keyboard::make_blacklist_keyboard(&blacklist))
                        .await?;
                }
                StartParameter::Regular => {
                    bot.send_markdown(msg.chat.id, Text::get_start_text())
                        .reply_to_message_id(msg.id)
                        .allow_sending_without_reply(true)
                        .await?;
                    bot.send_markdown(msg.chat.id, Text::get_main_text())
                        .reply_markup(Keyboard::make_main_keyboard())
                        .await?;
                }
                StartParameter::Help => {
                    bot.send_markdown(msg.chat.id, Text::get_help_text(user.is_admin))
                        .reply_markup(Keyboard::make_help_keyboard())
                        .await?;
                }
            },
            Self::Settings => {
                bot.send_markdown(msg.chat.id, Text::get_settings_text())
                    .reply_markup(Keyboard::make_settings_keyboard())
                    .await?;
            }
            Self::PopularTags => {
                let tags = database.get_popular_tags(20).await?;
                bot.send_markdown(msg.chat.id, Text::get_popular_tag_text(tags))
                    .await?;
            }
            Self::Stats => {
                let stats = database.get_stats().await?;
                bot.send_markdown(msg.chat.id, Text::get_stats_text(stats))
                    .await?;
            }
            Self::TagMe => {
                bot.send_chat_action(msg.chat.id, teloxide::types::ChatAction::Typing)
                    .await?;
                let sticker = database.get_random_sticker_to_tag().await?;
                if let Some(sticker) = sticker {
                    let tags = database.get_sticker_tags(sticker.id.clone()).await?;
                    let suggested_tags = suggest_tags(
                        &sticker.id,
                        bot.clone(),
                        tag_manager.clone(),
                        database.clone(),
                    )
                    .await?;
                    let set_name = database.get_set_name(sticker.id.clone()).await?;
                    bot.send_sticker(msg.chat.id, InputFile::file_id(sticker.file_id))
                        .reply_markup(Keyboard::make_tag_keyboard(
                            &tags,
                            &sticker.id,
                            &suggested_tags,
                            set_name,
                        ))
                        .reply_to_message_id(msg.id.0)
                        .allow_sending_without_reply(true)
                        .await?;
                } else {
                    bot.send_markdown(msg.chat.id, Markdown::escaped("No stickers to tag!"))
                        .await?;
                }
            }
            Self::ContinuousTagMode => {
                bot.send_markdown(
                    msg.chat.id,
                    Markdown::escaped("Choose a tag to apply to multiple stickers"),
                )
                .reply_markup(Keyboard::make_continuous_tag_keyboard())
                .await?;
            } // this kind of reply markup can be used to have users check the correctness of tags
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
