use crate::background_tasks::{SuggestTags, TaggingWorker};
use crate::bot::Bot;
use crate::database::Database;

use crate::tags::TagManager;
use crate::util::Emoji;
use anyhow::Result;
use itertools::Itertools;
use teloxide::requests::Requester;

use std::sync::Arc;

use super::{suggest_tags_2, ScoredTagSuggestion};

pub async fn suggest_tags(
    sticker_unique_id: &str,
    bot: Bot,
    tag_manager: Arc<TagManager>,
    database: Database,
    tagging_worker: TaggingWorker,
) -> Result<Vec<String>> {
    // TODO: redo these suggestion
    let suggested_tags = database
        .suggest_tags_for_sticker_based_on_other_stickers_in_set(sticker_unique_id.to_string())
        .await?;
    let max_count = suggested_tags
        .iter()
        .map(|tag| tag.count)
        .max()
        .unwrap_or(1);
    let mut suggested_tags = suggested_tags
        .into_iter()
        .map(|tag| ScoredTagSuggestion::new(tag.name, (tag.count as f64 / max_count as f64) * 0.5))
        .collect_vec(); // TODO: change scoring

    suggested_tags = ScoredTagSuggestion::merge(
        suggested_tags,
        tagging_worker
            .execute(SuggestTags::new(sticker_unique_id.to_string()))
            .await?,
    );

    // TODO: single call
    let set = database.get_set(sticker_unique_id.to_string()).await?;
    let sticker_tags = database
        .get_sticker_tags(sticker_unique_id.to_string())
        .await?;

    if let Some(set) = set {
        let set = bot.get_sticker_set(set.id).await?;
        let set_title = set.title;
        let set_name = set.name;
        let emojis = set
            .stickers
            .iter()
            .find(|sticker| sticker_unique_id == sticker.file.unique_id)
            .map(|sticker| sticker.emoji.clone().unwrap_or_default())
            .map(|emoji_string| Emoji::parse(&emoji_string))
            .unwrap_or_default();
        let suggestions =
            suggest_tags_2(&sticker_tags, tag_manager, emojis, &set_title, &set_name)?;
        suggested_tags = ScoredTagSuggestion::merge(suggested_tags, suggestions);
    }

    let result = Ok(suggested_tags
        .into_iter()
        .filter(|suggestion| !sticker_tags.contains(&suggestion.tag))
        .take(30)
        .map(|suggestion| suggestion.tag)
        .collect_vec());
    result
}
