use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use crate::{
    bot::BotError,
    database::{Database, FileAnalysisWithStickerId},
    inline::SimilarityAspect,
    sticker::{vec_u8_to_f32, IndexInput, ModelEmbedding, MyIndex, TopMatches},
};

use super::{Comm, State, Worker};

#[derive(Clone, Debug)]
pub struct AnalysisState {
    embedding_index: Arc<Mutex<MyIndex>>,
    color_index: Arc<Mutex<MyIndex>>,
    shape_index: Arc<Mutex<MyIndex>>,
    lookup: HashMap<u64, String>,
}

impl AnalysisState {
    async fn new(database: Database) -> Result<Arc<Self>, BotError> {
        let analysis = database.get_analysis_for_all_stickers().await?;

        let (embedding_index, color_index, shape_index, lookup) =
            tokio::task::spawn_blocking(move || recompute_index(analysis)).await??;

        Ok(Arc::new(Self {
            color_index,
            shape_index,
            embedding_index,
            lookup,
        }))
    }
    async fn needs_recomputation(&self, database: Database) -> Result<bool, BotError> {
        Ok(true) // TODO: change
    }
}

impl State for AnalysisState {
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

pub type AnalysisWorker = Worker<AnalysisState>;

pub struct Retrieve {
    aspect: SimilarityAspect,
    query: Vec<f32>,
    n: usize,
}

impl Retrieve {
    pub fn new(query: Vec<f32>, n: usize, aspect: SimilarityAspect) -> Self {
        Self { aspect, n, query }
    }

    async fn apply(&self, state: Arc<AnalysisState>) -> Result<TopMatches, BotError> {
        let embedding_index = match self.aspect {
            SimilarityAspect::Embedding => state.embedding_index.clone(),
            SimilarityAspect::Color => state.color_index.clone(),
            SimilarityAspect::Shape => state.shape_index.clone(),
        };
        let query = self.query.clone();
        let n = self.n.clone();
        let res = tokio::task::spawn_blocking(move || {
            let mut embedding_index = embedding_index
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            embedding_index.lookup(query, n)
        })
        .await??;
        let mut top_matches = TopMatches::new(self.n, 1.0);
        for res in res {
            let Some(id) = state.lookup.get(&res.label) else {
                continue;
            };
            top_matches.push(1.0 - f64::from(res.distance), id.to_string());
        }
        Ok(top_matches)
    }
}

impl Comm<AnalysisState> for Retrieve {
    type ReturnType = TopMatches;
    fn apply(
        &self,
        state: Arc<AnalysisState>,
    ) -> impl std::future::Future<Output = Result<Self::ReturnType, BotError>> + Send {
        Self::apply(&self, state)
    }
}

pub(super) fn recompute_index(
    analysis: Vec<FileAnalysisWithStickerId>,
) -> Result<
    (
        Arc<Mutex<MyIndex>>,
        Arc<Mutex<MyIndex>>,
        Arc<Mutex<MyIndex>>,
        HashMap<u64, String>,
    ),
    BotError,
> {
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
            });
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
    let embedding_index = Arc::new(Mutex::new(MyIndex::new(input_embedding)?));
    let color_index = Arc::new(Mutex::new(MyIndex::new(input_color)?));
    let shape_index = Arc::new(Mutex::new(MyIndex::new(input_shape)?));

    Ok((embedding_index, color_index, shape_index, lookup))
}
