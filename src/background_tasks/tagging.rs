use chrono::{DateTime, Utc};
use log::warn;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use tokio::sync::{mpsc, oneshot};

use crate::{
    bot::BotError,
    database::Database,
    sticker::{vec_u8_to_f32, IndexResult, ModelEmbedding, MyIndex},
    tags::{ScoredTagSuggestion, Tfidf},
};

use super::{recompute_index, Comm, State, Worker};

#[derive(Clone, Debug)]
pub struct TaggingState {
    embedding_index: Arc<Mutex<MyIndex>>,
    color_index: Arc<Mutex<MyIndex>>,
    lookup: HashMap<u64, String>,
    tfidf: Arc<Tfidf>,
    last_computed: DateTime<Utc>,
    database: Database, // TODO: dont put in state
}

impl TaggingState {
    async fn new(database: Database) -> Result<Arc<Self>, BotError> {
        let analysis = database.get_analysis_for_all_stickers_with_tags().await?;
        let (embedding_index, color_index, _, lookup) =
            tokio::task::spawn_blocking(move || recompute_index(analysis)).await??;
        let tfidf = Arc::new(Tfidf::generate(database.clone()).await?);
        let last_computed = chrono::Utc::now();

        Ok(Arc::new(Self {
            embedding_index,
            color_index,
            lookup,
            tfidf,
            last_computed,
            database,
        }))
    }
    async fn needs_recomputation(&self, database: Database) -> Result<bool, BotError> {
        Ok(chrono::Utc::now() - self.last_computed > chrono::Duration::minutes(5))
    }
}

impl State for TaggingState {
    fn generate(
        database: Database,
    ) -> impl std::future::Future<Output = Result<Arc<Self>, BotError>> + Send {
        Self::new(database)
    }
    fn needs_recomputation(
        &self,
        database: Database,
    ) -> impl std::future::Future<Output = Result<bool, BotError>> + Send {
        self.needs_recomputation(database)
    }
}

pub type TaggingWorker = Worker<TaggingState>;

pub struct SuggestTags {
    sticker_id: String,
}

impl SuggestTags {
    pub fn new(sticker_id: String) -> Self {
        Self { sticker_id }
    }

    async fn apply(&self, state: Arc<TaggingState>) -> Result<Vec<ScoredTagSuggestion>, BotError> {
        let database = state.database.clone();
        let analysis = database
            .get_analysis_for_sticker_id(self.sticker_id.clone())
            .await?
            .ok_or(anyhow::anyhow!("analysis missing"))?;
        let embedding_res = if let Some(embedding) = analysis.embedding {
            let query_embedding: ModelEmbedding = embedding.into();
            let embedding_query = query_embedding.into();
            let embedding_index = state.embedding_index.clone();
            tokio::task::spawn_blocking(move || {
                let mut embedding_index = embedding_index.lock().unwrap_or_else(PoisonError::into_inner);
                embedding_index.lookup(embedding_query, 20)
            })
            .await??
        } else {
            vec![]
        };
        let color_res = if let Some(histogram) = analysis.histogram {
            let color_query = vec_u8_to_f32(histogram);
            let color_index = state.color_index.clone();
            tokio::task::spawn_blocking(move || {
                let mut color_index = color_index.lock().unwrap_or_else(PoisonError::into_inner);
                color_index.lookup(color_query, 20)
            })
            .await??
        } else {
            vec![]
        };
        let tags_0 = database
            .get_sticker(self.sticker_id.to_string())
            .await
            .map(|sticker| {
                sticker
                    .map(|sticker| sticker.emoji.map(|emoji| state.tfidf.suggest_tags(emoji)))
                    .flatten()
            })
            .unwrap_or_else(|err| {
                warn!("{:?}", err);
                Some(vec![])
            })
            .unwrap_or_default();
        let tags_1 = get_all_tags_from_stickers(embedding_res, database.clone(), &state.lookup)
            .await
            .unwrap_or_default();
        let tags_2 = get_all_tags_from_stickers(color_res, database.clone(), &state.lookup)
            .await
            .unwrap_or_default();
        let merged = ScoredTagSuggestion::merge(ScoredTagSuggestion::merge(tags_0, tags_1), tags_2);
        Ok(merged)
    }
}

impl Comm<TaggingState> for SuggestTags {
    type ReturnType = Vec<ScoredTagSuggestion>;
    fn apply(
        &self,
        state: Arc<TaggingState>,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, BotError>> + Send {
        Self::apply(&self, state)
    }
}

async fn get_all_tags_from_stickers(
    index_result: Vec<IndexResult>,
    database: Database,
    lookup: &HashMap<u64, String>,
) -> Result<Vec<ScoredTagSuggestion>, BotError> {
    let mut res = Vec::new();
    let mut weight = 0.15; // results in a max of about 1.2 for 20 matches
    for entry in index_result {
        let Some(id) = lookup.get(&entry.label) else {
            continue;
        };
        let sticker_tags = database.get_sticker_tags(id.to_string()).await?;
        for tag in sticker_tags {
            res.push(ScoredTagSuggestion { tag, score: weight });
        }
        weight *= 0.9;
    }
    Ok(res)
}
