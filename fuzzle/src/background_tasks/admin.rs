use teloxide::{payloads::SendMessageSetters, types::UserId};

use crate::{
    bot::{Bot, BotExt, InternalError},
    database::Database,
    message::Keyboard,
    text::Text,
};

pub async fn send_daily_report(
    database: Database,
    bot: Bot,
    admin_id: UserId,
) -> Result<(), InternalError> {
    let counts = database.get_stats().await?;
    let stats = database.get_admin_stats().await?;
    let taggings = database.get_user_tagging_stats_24_hours().await?;

    bot
        .send_markdown(
            admin_id,
            Text::daily_report(counts, stats, taggings.clone()),
        )
        .reply_markup(Keyboard::daily_report(taggings)?)
        .await?;

    let new_sticker_sets = database.get_sticker_sets_added_24_hours().await?;

    for (set_name, added_by_user_id) in new_sticker_sets {
        let added_by_user_id = added_by_user_id.map(|user_id| UserId(user_id as u64));
        bot.send_markdown(admin_id, Text::new_set(&set_name))
            .reply_markup(Keyboard::new_set(added_by_user_id, &set_name)?)
            .await?;
    }

    Ok(())
}
