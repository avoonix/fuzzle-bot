use crate::bot::{Bot, BotError, BotExt, SendDocumentExt};
use crate::database::{export_database, Database};
use crate::text::Markdown;
use crate::worker::WorkerPool;
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

    #[command(description = "ADMIN queue all sets for refetching")]
    RefetchAllSets,

    #[command(description = "ADMIN export json")]
    ExportJson,
}

impl AdminCommand {
    #[must_use]
    pub fn list_visible() -> Vec<BotCommand> {
        [RegularCommand::bot_commands(), Self::bot_commands()].concat()
    }

    pub async fn execute(
        self,
        bot: Bot,
        msg: Message,
        database: Database,
        worker: WorkerPool,
    ) -> Result<(), BotError> {
        match self {
            Self::BanSet { set_name } => {
                let set_name = set_name.trim();
                if set_name.is_empty() {
                    bot.send_markdown(msg.chat.id, Markdown::escaped("missing set name"))
                        .await?;
                } else {
                    database.ban_set(set_name.to_string()).await?;
                    bot.send_markdown(msg.chat.id, Markdown::escaped("banned set"))
                        .await?;
                }
            }
            Self::RefetchAllSets => {
                worker.refetch_all_sets().await;
            }
            Self::ExportJson => {
                send_database_export_to_chat(msg.chat.id, database.clone(), bot.clone()).await?;
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
