use crate::background_tasks::{GetCategory, SuggestTags, TagManagerWorker, TfIdfWorker};
use crate::bot::{Bot, BotError, InternalError};
use crate::database::Database;

use crate::qdrant::VectorDatabase;
use crate::tags::{Category};
use crate::util::{Emoji, Required};
use futures::future::try_join_all;
use itertools::Itertools;
use teloxide::requests::Requester;
use tracing::Instrument;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::image_tag_similarity::suggest_closest_tags;
use super::implied::suggest_tags_by_reverse_implication;
use super::owner_tags::suggest_owners_tags;
use super::rules::get_default_rules;
use super::same_set_tags::{
    suggest_tags_from_same_set, suggest_tags_from_sets_with_same_owner,
    suggest_tags_from_sets_with_same_sticker_file,
};
use super::similar_stickers_tags::suggest_tags_from_similar_stickers;
use super::similar_tags::suggest_similar_tags;
use super::ScoredTagSuggestion;

// TODO: refactor the whole module
// - `TagSuggestions` should be a map
// - each `Vec<ScoredTagSuggestion>` should also be a map

#[tracing::instrument(skip(bot, tag_manager, database, tagging_worker, vector_db))]
pub async fn suggest_tags(
    sticker_id: &str,
    bot: Bot,
    tag_manager: TagManagerWorker,
    database: Database,
    tagging_worker: TfIdfWorker,
    vector_db: VectorDatabase,
) -> Result<Vec<String>, BotError> {
    let sticker = database.get_sticker_by_id(sticker_id).await?.required()?;
    let set = database
        .get_sticker_set_by_sticker_id(sticker_id)
        .await?
        .required()?;
    let sticker_tags = database.get_sticker_tags(sticker_id).await?;
    let emojis = database.get_sticker_emojis(sticker_id).await?;

    let suggestions = vec![
        if let Some(owner_id) = set.created_by_user_id {
            suggest_owners_tags(&database, owner_id).await?
        } else {
            vec![]
        },
        // db_based_sticker_tags_from_same_set:
        suggest_tags_from_same_set(&database, &set.id).await?,
        suggest_tags_from_sets_with_same_sticker_file(&database, &sticker.sticker_file_id).await?,
        if let Some(owner_id) = set.created_by_user_id {
            suggest_tags_from_sets_with_same_owner(&database, owner_id).await?
        } else {
            vec![]
        },
        // worker_based_tf_idf:
        tagging_worker
            .execute(SuggestTags::new(sticker_id.to_string()))
            .await?,
        // clip_and_db_based_tags_from_similar_stickers:
        suggest_tags_from_similar_stickers(
            &database,
            &vector_db,
            &sticker.sticker_file_id,
            0.7,
            200,
        )
        .await?,
        suggest_tags_from_similar_stickers(
            &database,
            &vector_db,
            &sticker.sticker_file_id,
            0.9,
            30,
        )
        .await?,
        // clip_text_embedding_based_on_existing_tags:
        suggest_similar_tags(
            &database,
            &vector_db,
            tag_manager.clone(),
            sticker_tags.as_slice(),
        )
        .await?,
        // tag_manager_based_reverse_implications:
        suggest_tags_by_reverse_implication(&sticker_tags, tag_manager.clone()).await?,
        // image_to_tag_similarity_based:
        suggest_closest_tags(
            &database,
            &vector_db,
            tag_manager.clone(),
            &sticker.sticker_file_id,
        )
        .await?,
        // static_rule_based_emoji_and_set_name:
        get_default_rules() // TODO: those are re-parsed every time!
            .suggest_tags(emojis, &set.title.unwrap_or_default(), &set.id),
    ];
    Ok(combine_suggestions_alt_1(
        suggestions,
        sticker_tags,
        tag_manager,
    ).await?)
}

// #[tracing::instrument(skip(tag_manager))]
// fn combine_suggestions(
//     suggestions: TagSuggestions,
//     sticker_tags: Vec<String>,
//     tag_manager: TagManagerWorker,
// ) -> Vec<String> {
//     // TODO: weighting, etc:
//     let suggested_tags = suggestions
//         .db_based_sticker_tags_from_same_set
//         .into_iter()
//         .chain(suggestions.worker_based_tf_idf)
//         .chain(suggestions.clip_and_db_based_tags_from_similar_stickers)
//         .chain(suggestions.clip_text_embedding_based_on_existing_tags)
//         .chain(suggestions.tag_manager_based_reverse_implications)
//         .chain(suggestions.static_rule_based_emoji_and_set_name)
//         .chain(suggestions.static_default_tags)
//         .chain(suggestions.image_to_tag_similarity_based)
//         .collect_vec();
//     let suggested_tags = ScoredTagSuggestion::merge(suggested_tags, vec![]);

//     let mut limits = HashMap::new();
//     limits.insert(Category::General, 10);
//     limits.insert(Category::Species, 4);
//     limits.insert(Category::Meta, 5);

//     let result = suggested_tags
//         .into_iter()
//         .filter(|suggestion| !sticker_tags.contains(&suggestion.tag))
//         .filter(|suggestion| {
//             tag_manager
//                 .get_category(&suggestion.tag)
//                 .map(|category| {
//                     let entry = limits.entry(category).or_insert(2);
//                     *entry -= 1;
//                     *entry >= 0
//                 })
//                 .unwrap_or_default()
//         })
//         .take(16)
//         .map(|suggestion| suggestion.tag)
//         .collect_vec();
//     result
// }

// #[tracing::instrument(skip(tag_manager))]
// fn combine_suggestions_alt_1(
//     suggestions: Vec<Vec<ScoredTagSuggestion>>,
//     sticker_tags: Vec<String>,
//     tag_manager: TagManagerWorker,
// ) -> Vec<String> {
//     let suggestion_vec = suggestions
//         .into_iter()
//         .map(|s| {
//             let ranked: HashMap<_, _> = ScoredTagSuggestion::merge(
//                 ScoredTagSuggestion::add_implications(s, tag_manager.clone()),
//                 vec![],
//             )
//             .into_iter()
//             .enumerate()
//             .map(|(index, tag)| (tag.tag, index))
//             .collect();
//             ranked
//         })
//         .filter(|s| s.len() > 0)
//         .collect_vec();

//     let mut all_tags: HashMap<_, _> = suggestion_vec
//         .iter()
//         .flat_map(|s| s.iter().map(|(tag, _)| (tag.to_string(), 0)))
//         .collect();
//     for tags in suggestion_vec {
//         for (tag, value) in all_tags.iter_mut() {
//             let len = tags.len();
//             let score = tags.get(tag).unwrap_or(&len); // score = index in the original suggestion, or the length of the original suggestion if it does not exist
//             *value += (score * 100) / len; // every suggester can give a score between 0 and 100
//         }
//     }

//     let mut limits = HashMap::new();
//     limits.insert(Category::General, 15);
//     limits.insert(Category::Species, 5);
//     limits.insert(Category::Meta, 5);

//     let result = all_tags
//         .into_iter()
//         .sorted_unstable_by_key(|it| it.1)
//         .map(|it| it.0)
//         .filter(|suggestion| !sticker_tags.contains(&suggestion))
//         .filter(|suggestion| {
//             tag_manager
//                 .get_category(&suggestion)
//                 .map(|category| {
//                     let entry = limits.entry(category).or_insert(2);
//                     *entry -= 1;
//                     *entry >= 0
//                 })
//                 .unwrap_or_default()
//         })
//         .take(20)
//         // .map(|suggestion| suggestion.tag)
//         .collect_vec();
//     result
// }

#[tracing::instrument(skip(tag_manager))]
async fn combine_suggestions_alt_1(
    suggestions: Vec<Vec<ScoredTagSuggestion>>,
    sticker_tags: Vec<String>,
    tag_manager: TagManagerWorker,
) -> Result<Vec<String>, InternalError> {
    let suggestion_vec = try_join_all(suggestions
        .into_iter()
        .map(|s| async {
            Ok::<_, InternalError>(ScoredTagSuggestion::merge(
                ScoredTagSuggestion::add_implications(s, tag_manager.clone()).await?,
                vec![],
            )
            .into_iter()
            .take(30)
            .collect_vec())
        })).await?
        .into_iter()
        .filter(|s| s.len() > 0)
        .collect_vec();

    let mut all_tags: HashMap<_, _> = suggestion_vec
        .iter()
        .flatten()
        .map(|suggestion| (suggestion.tag.clone(), 0))
        .collect();
    for tags in suggestion_vec {
        for ScoredTagSuggestion { tag, .. } in tags {
            *all_tags.entry(tag).or_default() += 1;
        }
    }

    let all_tags = filter(all_tags, sticker_tags.clone());

    let mut limits = HashMap::new();
    limits.insert(Category::General, 15);
    limits.insert(Category::Species, 5);
    limits.insert(Category::Meta, 5);

    let result = try_join_all(all_tags
        .into_iter()
        .sorted_unstable_by_key(|it| -it.1)
        .map(|it| it.0)
        .filter(|suggestion| !sticker_tags.contains(&suggestion))
        .map(|suggestion| async {
            let category = tag_manager .execute(GetCategory::new(suggestion.to_string())).await?;
            Ok::<_, InternalError>((suggestion, category))
        }))
        .await?
        .into_iter()
        .filter(|(suggestion, category)| {
            category
                .map(|category| {
                    let entry = limits.entry(category).or_insert(2);
                    *entry -= 1;
                    *entry >= 0
                })
                .unwrap_or_default()
        })
        .take(20)
        .map(|(suggestion, _)| suggestion)
        .collect_vec();
    Ok(result)
}

fn filter(mut all_tags: HashMap<String, i32>, sticker_tags: Vec<String>) -> HashMap<String, i32> {
    let default_tags = [
        "ych_(character)",
        "questionable",
        "explicit",
        "safe",
        "solo",
        "diaper",
        "duo",
        "watersports",
        "young",
        "vore",
        "scat",
        "gore",
        "attribution",
        "male",
        "female",
        "ambiguous_gender",
    ];
    for tag in default_tags {
        all_tags.entry(tag.to_string()).or_default(); // add default tags with score 0
    }

    all_tags
}
