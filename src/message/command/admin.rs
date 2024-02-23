use crate::background_tasks::send_daily_report;
use crate::bot::{Bot, BotError, BotExt, RequestContext, SendDocumentExt};
use crate::database::{export_database, Database};
use crate::message::Keyboard;
use crate::text::Markdown;
use crate::Config;
use flate2::read::GzEncoder;
use flate2::Compression;
use log::info;
use std::io::prelude::*;
use teloxide::types::{BotCommand, InputFile};

use teloxide::{prelude::*, utils::command::BotCommands};

use super::user::RegularCommand;

#[derive(BotCommands, Debug)]
#[command(rename_rule = "lowercase", description = "Admin commands")]
pub enum AdminCommand {
    #[command(description = "ADMIN ban a set (set name is case sensitive)")]
    BanSet { set_name: String },

    #[command(description = "ADMIN export json")]
    ExportJson,

    #[command(description = "ADMIN send daily report immediately")]
    Report,

    #[command(description = "ADMIN ui")]
    Ui,
}

impl AdminCommand {
    #[must_use]
    pub fn list_visible() -> Vec<BotCommand> {
        [RegularCommand::bot_commands(), Self::bot_commands()].concat()
    }

    pub async fn execute(
        self,
        msg: Message,
        request_context: RequestContext,
    ) -> Result<(), BotError> {
        match self {
            Self::BanSet { set_name } => {
                let set_name = set_name.trim();
                if set_name.is_empty() {
                    request_context.bot.send_markdown(msg.chat.id, Markdown::escaped("missing set name"))
                        .await?;
                } else {
                    request_context.database.ban_set(set_name.to_string()).await?;
                    request_context.bot.send_markdown(msg.chat.id, Markdown::escaped("banned set"))
                        .await?;
                }
            }
            Self::ExportJson => {
                send_database_export_to_chat(msg.chat.id, request_context.database.clone(), request_context.bot.clone()).await?;
            }
            Self::Ui => {
                request_context.bot.send_markdown(msg.chat.id, Markdown::escaped("Log in with this button"))
                    .reply_markup(Keyboard::ui(request_context.config.domain_name.clone())?)
                    .await?;
            }
            Self::Report => {
                send_daily_report(request_context.database, request_context.bot, request_context.config.get_admin_user_id()).await?;
            }
        }

        Ok(())
    }
}

pub async fn send_database_export_to_chat(
    chat_id: ChatId,
    database: Database,
    bot: Bot,
) -> Result<(), BotError> {
    let data = export_database(database).await?;
    let data = serde_json::to_vec(&data)?; // TODO: stream?
    let mut gz = GzEncoder::new(&*data, Compression::best());
    let mut buffer = Vec::new();
    gz.read_to_end(&mut buffer)?;
    let kbytes = data.len() / 1024;
    info!("{} KiB exported", kbytes);
    bot.send_document(
        chat_id,
        InputFile::memory(data).file_name("stickers.json.gz"),
    )
    .markdown_caption(Markdown::escaped("list of all sticker sets"))
    .await?;
    Ok(())
}
