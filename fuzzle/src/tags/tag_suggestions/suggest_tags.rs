use crate::background_tasks::{SuggestTags, TaggingWorker};
use crate::bot::{Bot, BotError};
use crate::database::Database;

use crate::qdrant::VectorDatabase;
use crate::tags::{Category, TagManager};
use crate::util::{Emoji, Required};
use itertools::Itertools;
use teloxide::requests::Requester;
use tracing::Instrument;

use std::collections::HashMap;
use std::sync::Arc;

use super::defaults::suggest_default_tags;
use super::image_tag_similarity::suggest_closest_tags;
use super::implied::suggest_tags_by_reverse_implication;
use super::rules::get_default_rules;
use super::same_set_tags::suggest_tags_from_same_set;
use super::similar_stickers_tags::suggest_tags_from_similar_stickers;
use super::similar_tags::suggest_similar_tags;
use super::ScoredTagSuggestion;

// TODO: refactor the whole module
// - `TagSuggestions` should be a map
// - each `Vec<ScoredTagSuggestion>` should also be a map

#[derive(Debug)]
pub struct TagSuggestions {
    pub db_based_sticker_tags_from_same_set: Vec<ScoredTagSuggestion>,
    pub worker_based_tf_idf: Vec<ScoredTagSuggestion>,
    pub clip_and_db_based_tags_from_similar_stickers: Vec<ScoredTagSuggestion>,
    pub clip_text_embedding_based_on_existing_tags: Vec<ScoredTagSuggestion>,
    pub tag_manager_based_reverse_implications: Vec<ScoredTagSuggestion>,
    pub static_rule_based_emoji_and_set_name: Vec<ScoredTagSuggestion>,
    pub static_default_tags: Vec<ScoredTagSuggestion>,
    pub image_to_tag_similarity_based: Vec<ScoredTagSuggestion>,
}

#[tracing::instrument(skip(bot, tag_manager, database, tagging_worker, vector_db))]
pub async fn suggest_tags(
    sticker_id: &str,
    bot: Bot,
    tag_manager: Arc<TagManager>,
    database: Database,
    tagging_worker: TaggingWorker,
    vector_db: VectorDatabase,
) -> Result<Vec<String>, BotError> {
    let sticker = database.get_sticker_by_id(sticker_id).await?.required()?;
    let set = database
        .get_sticker_set_by_sticker_id(sticker_id)
        .await?.required()?;
    let sticker_tags = database.get_sticker_tags(sticker_id).await?;
    let emojis = database.get_sticker_emojis(sticker_id).await?;

    let suggestions = TagSuggestions {
        db_based_sticker_tags_from_same_set: suggest_tags_from_same_set(&database, &set.id).await?,
        worker_based_tf_idf: tagging_worker
            .execute(SuggestTags::new(sticker_id.to_string()))
            .await?,
        clip_and_db_based_tags_from_similar_stickers: suggest_tags_from_similar_stickers(
            &database,
            &vector_db,
            &sticker.sticker_file_id,
        )
        .await?,
        clip_text_embedding_based_on_existing_tags: suggest_similar_tags(
            &database,
            &vector_db,
            tag_manager.clone(),
            sticker_tags.as_slice(),
        )
        .await?,
        tag_manager_based_reverse_implications: suggest_tags_by_reverse_implication(
            &sticker_tags,
            tag_manager.clone(),
        ),
image_to_tag_similarity_based: suggest_closest_tags(&database, &vector_db, tag_manager.clone(), &sticker.sticker_file_id).await?,
        static_rule_based_emoji_and_set_name: get_default_rules().suggest_tags(
            emojis,
            &set.title.unwrap_or_default(),
            &set.id,
        ),
        static_default_tags: suggest_default_tags(),
    };
    Ok(combine_suggestions_alt_1(suggestions, sticker_tags, tag_manager))
}

#[tracing::instrument(skip(tag_manager))]
fn combine_suggestions(
    suggestions: TagSuggestions,
    sticker_tags: Vec<String>,
    tag_manager: Arc<TagManager>,
) -> Vec<String> {
    // TODO: weighting, etc:
    let suggested_tags = suggestions
        .db_based_sticker_tags_from_same_set
        .into_iter()
        .chain(suggestions.worker_based_tf_idf)
        .chain(suggestions.clip_and_db_based_tags_from_similar_stickers)
        .chain(suggestions.clip_text_embedding_based_on_existing_tags)
        .chain(suggestions.tag_manager_based_reverse_implications)
        .chain(suggestions.static_rule_based_emoji_and_set_name)
        .chain(suggestions.static_default_tags)
        .chain(suggestions.image_to_tag_similarity_based)
        .collect_vec();
    let suggested_tags = ScoredTagSuggestion::merge(suggested_tags, vec![]);

    let mut limits = HashMap::new();
    limits.insert(Category::General, 10);
    limits.insert(Category::Species, 4);
    limits.insert(Category::Meta, 5);

    let result = suggested_tags
        .into_iter()
        .filter(|suggestion| !sticker_tags.contains(&suggestion.tag))
        .filter(|suggestion| {
            tag_manager
                .get_category(&suggestion.tag)
                .map(|category| {
                    let entry = limits.entry(category).or_insert(2);
                    *entry -= 1;
                    *entry >= 0
                })
                .unwrap_or_default()
        })
        .take(16)
        .map(|suggestion| suggestion.tag)
        .collect_vec();
    result
}

#[tracing::instrument(skip(tag_manager))]
fn combine_suggestions_alt_1(
    suggestions: TagSuggestions,
    sticker_tags: Vec<String>,
    tag_manager: Arc<TagManager>,
) -> Vec<String> {
    // TODO: limit all suggestions to like 30?
    let suggestion_vec = vec![
        suggestions.db_based_sticker_tags_from_same_set,
        suggestions.worker_based_tf_idf,
        suggestions.clip_and_db_based_tags_from_similar_stickers,
        suggestions.clip_text_embedding_based_on_existing_tags,
        suggestions.tag_manager_based_reverse_implications,
        suggestions.static_rule_based_emoji_and_set_name,
        suggestions.static_default_tags,
        suggestions.image_to_tag_similarity_based,
    ];
    let suggestion_vec = suggestion_vec.into_iter().map(|s| {
        let ranked: HashMap<_, _> = ScoredTagSuggestion::merge(ScoredTagSuggestion::add_implications(s, tag_manager.clone()), vec![])
            .into_iter()
            .enumerate()
            .map(|(index, tag)| (tag.tag, index))
            .collect();
        ranked
    }).filter(|s| s.len() > 0).collect_vec();

    let mut all_tags : HashMap<_, _> = suggestion_vec.iter().flat_map(|s| s.iter().map(|(tag, _)| (tag.to_string(), 0))).collect();
    for tags in suggestion_vec {
        for (tag, value) in all_tags.iter_mut() {
            let len = tags.len();
            let score = tags.get(tag).unwrap_or(&len); // score = index in the original suggestion, or the length of the original suggestion if it does not exist
            *value += (score * 100) / len; // every suggester can give a score between 0 and 100
        }
    }

    let mut limits = HashMap::new();
    limits.insert(Category::General, 15);
    limits.insert(Category::Species, 5);
    limits.insert(Category::Meta, 5);

    let result = all_tags
        .into_iter()
        .sorted_unstable_by_key(|it| it.1)
        .map(|it| it.0)
        .filter(|suggestion| !sticker_tags.contains(&suggestion))
        .filter(|suggestion| {
            tag_manager
                .get_category(&suggestion)
                .map(|category| {
                    let entry = limits.entry(category).or_insert(2);
                    *entry -= 1;
                    *entry >= 0
                })
                .unwrap_or_default()
        })
        .take(20)
        // .map(|suggestion| suggestion.tag)
        .collect_vec();
    result
}
