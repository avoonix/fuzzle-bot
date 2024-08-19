use itertools::Itertools;
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
    let new_sticker_sets = new_sticker_sets.into_iter().sorted_by_key(|(_, user)| user.clone()).chunk_by(|(_, user)| user.clone()).into_iter().map(|(user_id, chunk)| 
(user_id.map(|user_id| UserId(user_id as u64)), chunk.map(|(set_name, _)| set_name).collect_vec())
).collect_vec();

    for (user_id, set_names) in new_sticker_sets {
        for set_names in set_names.chunks(50) {
            bot.send_markdown(admin_id, Text::new_set(set_names))
                .reply_markup(Keyboard::new_sets(user_id, set_names)?)
                .await?;
        }
    }

    Ok(())
}
