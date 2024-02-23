use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use log::{error, warn};
use tokio::sync::{mpsc, oneshot};

use crate::{
    bot::BotError,
    database::{Database, FileAnalysisWithStickerId},
    inline::SimilarityAspect,
    sticker::{vec_u8_to_f32, IndexInput, ModelEmbedding, MyIndex, TopMatches},
};

#[derive(Clone, Debug)]
pub struct AnalysisWorker {
    tx: mpsc::Sender<Command>,
}

type Responder<T> = oneshot::Sender<Result<T, BotError>>;

#[derive(Debug)]
pub enum Command {
    Recompute,
    Get {
        aspect: SimilarityAspect,
        query: Vec<f32>,
        n: usize,
        resp: Responder<TopMatches>,
    },
}

impl AnalysisWorker {
    pub fn start(database: Database) -> Self {
        let (tx, mut rx) = mpsc::channel(10);
        tokio::spawn(async move {
            let analysis = database
                .get_analysis_for_all_stickers()
                .await
                .unwrap_or_else(|err| {
                    error!("{}", err);
                    vec![]
                });
            let (mut embedding_index, mut color_index, mut shape_index, mut lookup) =
                tokio::task::spawn_blocking(move || recompute_index(analysis))
                    .await
                    .unwrap();

            while let Some(cmd) = rx.recv().await {
                match cmd {
                    Command::Recompute => {
                        let analysis = database
                            .get_analysis_for_all_stickers()
                            .await
                            .unwrap_or_default();
                        if !analysis.is_empty() {
                            (embedding_index, color_index, shape_index, lookup) =
                                tokio::task::spawn_blocking(move || recompute_index(analysis))
                                    .await
                                    .unwrap();
                        }
                    }
                    Command::Get {
                        aspect,
                        query,
                        n,
                        resp,
                    } => {
                        let embedding_index = match aspect {
                            SimilarityAspect::Embedding => embedding_index.clone(),
                            SimilarityAspect::Color => color_index.clone(),
                            SimilarityAspect::Shape => shape_index.clone(),
                        };
                        let res = tokio::task::spawn_blocking(move || {
                            let mut embedding_index = embedding_index.lock().unwrap();
                            embedding_index.lookup(query, n).unwrap()
                        })
                        .await
                        .unwrap();
                        let mut top_matches = TopMatches::new(n, 1.0);
                        for res in res {
                            let Some(id) = lookup.get(&res.label) else {
                                continue;
                            };
                            top_matches.push(1.0 - res.distance as f64, id.to_string());
                        }
                        resp.send(Ok(top_matches)).unwrap();
                    }
                }
            }
        });

        Self { tx }
    }

    /// does not wait for completion
    pub async fn recompute(&self) -> Result<(), BotError> {
        self.tx.send(Command::Recompute).await?;
        Ok(())
    }

    pub async fn retrieve(
        &self,
        query: Vec<f32>,
        n: usize,
        aspect: SimilarityAspect,
    ) -> Result<TopMatches, BotError> {
        let (resp, receive) = oneshot::channel();
        self.tx.send(Command::Get {
            query,
            aspect,
            n,
            resp,
        }).await?;
        receive.await?
    }
}

pub(super) fn recompute_index(
    analysis: Vec<FileAnalysisWithStickerId>,
) -> (
    Arc<Mutex<MyIndex>>,
    Arc<Mutex<MyIndex>>,
    Arc<Mutex<MyIndex>>,
    HashMap<u64, String>,
) {
    let mut lookup = HashMap::new();
    let mut input_embedding = Vec::new();
    let mut input_color = Vec::new();
    let mut input_shape = Vec::new();
    let mut id = 1;
    for analysis in analysis {
        let Some(sticker_id) = analysis.sticker_id else {
            continue;
        };
        lookup.insert(id, sticker_id);
        if let Some(embedding) = analysis.embedding {
            let embedding = ModelEmbedding::from(embedding);
            let embedding: Vec<f32> = embedding.into();
            input_embedding.push(IndexInput {
                label: id,
                vec: embedding,
            })
        }
        if let Some(histogram) = analysis.histogram {
            let histogram = vec_u8_to_f32(histogram);
            input_color.push(IndexInput {
                label: id,
                vec: histogram,
            });
        }
        if let Some(visual_hash) = analysis.visual_hash {
            let visual_hash = vec_u8_to_f32(visual_hash);
            input_shape.push(IndexInput {
                label: id,
                vec: visual_hash,
            });
        }

        id += 1;
    }
    let embedding_index = Arc::new(Mutex::new(MyIndex::new(input_embedding).unwrap()));
    let color_index = Arc::new(Mutex::new(MyIndex::new(input_color).unwrap()));
    let shape_index = Arc::new(Mutex::new(MyIndex::new(input_shape).unwrap()));

    (embedding_index, color_index, shape_index, lookup)
}
