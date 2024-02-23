use teloxide::{payloads::SendMessageSetters, types::UserId};

use crate::{
    bot::{Bot, BotError, BotExt},
    database::Database,
    message::Keyboard,
    text::Text,
};

pub async fn send_daily_report(
    database: Database,
    bot: Bot,
    admin_id: UserId,
) -> Result<(), BotError> {
    let counts = database.get_stats().await?;
    let stats = database.get_admin_stats().await?;
    let taggings = database.get_user_tagging_stats_24_hours().await?;

    let result = bot
        .send_markdown(
            admin_id,
            Text::daily_report(counts, stats, taggings.clone()),
        )
        .reply_markup(Keyboard::daily_report(taggings)?)
        .await?;
    Ok(())
}

#[derive(Debug)]
pub enum AdminMessage {
    NewUser,
    StickerSetAdded { set_name: String },
}

pub async fn send_message_to_admin(
    msg: AdminMessage,
    source_user: UserId,
    bot: Bot,
    admin_id: UserId,
) -> Result<(), BotError> {
    let keyboard;
    let message;

    match msg {
        AdminMessage::StickerSetAdded { set_name } => {
            message = Text::new_set(&set_name);
            keyboard = Keyboard::new_set(source_user, &set_name)?;
        }
        AdminMessage::NewUser => {
            message = Text::new_user(source_user);
            keyboard = Keyboard::new_user(source_user);
        }
    };

    bot.send_markdown(admin_id, message)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}
