use std::{
    collections::HashMap,
    sync::{Arc},
};
use log::error;

use tokio::sync::{mpsc, oneshot};

use crate::{
    bot::BotError,
    database::{Database},
    sticker::{vec_u8_to_f32, IndexResult, ModelEmbedding},
    tags::{ScoredTagSuggestion, Tfidf},
};

use super::recompute_index;

#[derive(Clone, Debug)]
pub struct TaggingWorker {
    tx: mpsc::Sender<TaggingWorkerCommand>,
}

type Responder<T> = oneshot::Sender<Result<T, BotError>>;

#[derive(Debug)]
pub enum TaggingWorkerCommand {
    MaybeRecompute,
    SuggestTags {
        sticker_id: String,
        resp: Responder<Vec<ScoredTagSuggestion>>,
    },
}

impl TaggingWorker {
    pub fn start(database: Database) -> Self {
        let (tx, mut rx) = mpsc::channel(10);
        tokio::spawn(async move {
            let analysis = database
                .get_analysis_for_all_stickers_with_tags()
                .await
                .unwrap_or_else(|err| {
                    error!("{}", err);
                    vec![]
                });
            let mut tfidf = Arc::new(Tfidf::generate(database.clone()).await.unwrap());
            let (mut embedding_index, mut color_index, _, mut lookup) =
                tokio::task::spawn_blocking(move || recompute_index(analysis))
                    .await
                    .unwrap();
            let mut last_computed = chrono::Utc::now();

            while let Some(cmd) = rx.recv().await {
                match cmd {
                    TaggingWorkerCommand::MaybeRecompute => {
                        if chrono::Utc::now() - last_computed > chrono::Duration::minutes(5) {
                            let analysis = database
                                .get_analysis_for_all_stickers_with_tags()
                                .await
                                .unwrap();
                            (embedding_index, color_index, _, lookup) =
                                tokio::task::spawn_blocking(move || recompute_index(analysis))
                                    .await
                                    .unwrap();
                            tfidf = Arc::new(Tfidf::generate(database.clone()).await.unwrap());
                            last_computed = chrono::Utc::now();
                        }
                    }
                    TaggingWorkerCommand::SuggestTags { sticker_id, resp } => {
                        let analysis = database
                            .get_analysis_for_sticker_id(sticker_id.clone())
                            .await;
                        let analysis = match analysis {
                            Err(err) => {
                                resp.send(Err(err.into())).unwrap();
                                continue;
                            }
                            Ok(None) => {
                                resp.send(Err(anyhow::anyhow!("missing analysis").into())).unwrap();
                                continue;
                            }
                            Ok(Some(value)) => value,
                        };
                        let embedding_res = if let Some(embedding) = analysis.embedding {
                            let query_embedding: ModelEmbedding = embedding.into();
                            let embedding_query = query_embedding.into();
                            let embedding_index = embedding_index.clone();
                            tokio::task::spawn_blocking(move || {
                                let mut embedding_index = embedding_index.lock().unwrap();
                                embedding_index.lookup(embedding_query, 20).unwrap()
                            })
                            .await
                            .unwrap()
                        } else {
                            vec![]
                        };
                        let color_res = if let Some(histogram) = analysis.histogram {
                            let color_query = vec_u8_to_f32(histogram);
                            let color_index = color_index.clone();
                            tokio::task::spawn_blocking(move || {
                                let mut color_index = color_index.lock().unwrap();
                                color_index.lookup(color_query, 20).unwrap()
                            })
                            .await
                            .unwrap()
                        } else {
                            vec![]
                        };
                        let sticker = database.get_sticker(sticker_id.to_string()).await.unwrap();
                        let sticker = sticker.unwrap();
                        let tags_0 = tfidf.suggest_tags(sticker.emoji.unwrap());
                        let tags_1 =
                            get_all_tags_from_stickers(embedding_res, database.clone(), &lookup)
                                .await
                                .unwrap_or_default();
                        let tags_2 =
                            get_all_tags_from_stickers(color_res, database.clone(), &lookup)
                                .await
                                .unwrap_or_default();
                        let merged = ScoredTagSuggestion::merge(
                            ScoredTagSuggestion::merge(tags_0, tags_1),
                            tags_2,
                        );
                        resp.send(Ok(merged)).unwrap();
                    }
                }
            }
        });

        Self { tx }
    }

    /// does not wait for completion
    pub async fn maybe_recompute(&self) -> Result<(), BotError> {
        self.tx.send(TaggingWorkerCommand::MaybeRecompute).await?;
        Ok(())
    }

    pub async fn suggest(&self, sticker_id: String) -> Result<Vec<ScoredTagSuggestion>, BotError> {
        let (resp, receive) = oneshot::channel();
        self.tx
            .send(TaggingWorkerCommand::SuggestTags { sticker_id, resp })
            .await?;
        receive.await?
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
