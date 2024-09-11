use crate::background_tasks::send_daily_report;
use crate::bot::{Bot, BotError, BotExt, InternalError, RequestContext, SendDocumentExt};
use crate::database::{export_database, Database, StickerIdStickerFileId};
use crate::message::Keyboard;
use crate::sticker::generate_merge_image;
use crate::text::Markdown;
use crate::util::Required;

use flate2::read::GzEncoder;
use flate2::Compression;
use tracing::info;
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

    #[command(description = "ADMIN get pending moderation tasks")]
    Tasks,

    #[command(description = "ADMIN ui")]
    Ui,

    #[command(description = "ADMIN merge queue")]
    MergeQueue,
}

impl AdminCommand {
    #[must_use]
    pub fn list_visible() -> Vec<BotCommand> {
        [RegularCommand::bot_commands(), Self::bot_commands()].concat()
    }

    #[tracing::instrument(skip(self, msg, request_context))]
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
                    request_context.database.delete_sticker_set(set_name).await?;
                    request_context.database.ban_set(set_name, None).await?;
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
            Self::Tasks => {
                send_daily_report(request_context.database, request_context.bot, request_context.config.get_admin_user_id()).await?;
            }
            Self::MergeQueue => {
                for _ in 0..10 {
                    send_merge_queue(msg.chat.id, request_context.clone()).await?;
                }
            }
        }

        Ok(())
    }
}

pub async fn send_merge_queue(chat_id: ChatId, request_context: RequestContext) -> Result<(), BotError> {
    // TODO: not random
    // TODO: maybe spawn without waiting since this may take a while
    let Some((file_id_a, file_id_b)) = request_context.database.get_random_potential_merge_file_ids().await? else {
        request_context.bot.send_markdown(chat_id, Markdown::escaped("No more potential merges :3")).await?;
        return Ok(());
    };

                request_context
                    .bot
                    .send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
                    .await?;

    let result = request_context.database.get_some_sticker_ids_for_sticker_file_ids(vec![file_id_a, file_id_b]).await?;
    let mut result = result.into_iter();
    let StickerIdStickerFileId {sticker_id: a, ..} = result.next().required()?;
    let StickerIdStickerFileId { sticker_id: b, ..} = result.next().required()?;
    let set_a = request_context.database.get_sticker_set_by_sticker_id(&a).await?.required()?;
    let set_b = request_context.database.get_sticker_set_by_sticker_id(&b).await?.required()?;

    let buf = generate_merge_image(
        &a,
        &b,
        request_context.database.clone(),
        request_context.bot.clone(),
    )
    .await?;

    request_context.bot.send_document(
        chat_id,
        InputFile::memory(buf).file_name("comparison.png"),
    )
    .markdown_caption(Markdown::escaped("TODO: some content"))
    .reply_markup(Keyboard::merge(&a, &b, &set_a.id, &set_b.id)?)
    .await?;
Ok(())
}

pub async fn send_database_export_to_chat(
    chat_id: ChatId,
    database: Database,
    bot: Bot,
) -> Result<(), InternalError> {
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
