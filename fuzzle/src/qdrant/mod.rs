mod error;

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use error::SensibleQdrantErrorExt;
pub use error::VectorDatabaseError;
use itertools::Itertools;
use qdrant_client::prelude::*;
use qdrant_client::qdrant::vectors_config::Config;
use qdrant_client::qdrant::{
    Condition, CreateCollection, Datatype, Filter, LookupLocation, PayloadIncludeSelector,
    PointsIdsList, RecommendPoints, RecommendResponse, RecommendStrategy, ScoredPoint,
    ScrollPoints, SearchPoints, VectorParams, VectorParamsMap, VectorsConfig, WithPayloadSelector,
};
use serde_json::json;
use uuid::Uuid;

use crate::inline::SimilarityAspect;
use crate::sticker::{cosine_similarity, vec_u8_to_f32};

const TAG_COLLECTION_NAME: &str = "tag_v0";

const STICKER_COLLECTION_NAME: &str = "sticker_v0";
const STICKER_COLLECTION_SIZE: u64 = 768;
const STICKER_COLLECTION_DISTANCE: Distance = Distance::Cosine;

// - initialize qdrant collection
// - when crawling sticker sets, check if (all) stickers are already present in the collection
// - if stickers are missing, embed and insert them into the collection
// - offer endpoint to search
// - offer endpoint to delete sticker embeddings

// TODO: create method get_missing_embeddings that takes a set id, gets all the stickers from the main db
//       gets all the embeddings from qdrant (by file id), and checks if there is anything missing
//       another method embed_stickers embeds all the stickers returned by get_missing_embeddings
//       get_missing embeddings is called for every sticker set in the periodic sticker pack
//       update/fetch function
//       but if this is a sticker sent by a user, instead call the embed_stickers method on the
//       user sticker directly, and then call the other two functions in the set fetch method
//       that gets called after the sticker is inserted

#[derive(Clone)]
pub struct VectorDatabase {
    client: Arc<QdrantClient>,
}

impl Debug for VectorDatabase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorDatabase").finish() // qdrant client doesn't implement debug
    }
}

impl VectorDatabase {
    #[tracing::instrument(name = "VectorDatabase::new", err(Debug))]
    pub async fn new(url: &str) -> Result<Self, VectorDatabaseError> {
        let client = QdrantClient::from_url(url).build()?;
        let client = Self {
            client: Arc::new(client),
        };
        client.init_sticker_collection().await?;
        client.init_tag_collection().await?;
        Ok(client)
    }

    #[tracing::instrument(skip(self), err(Debug))]
    async fn init_tag_collection(&self) -> Result<(), VectorDatabaseError> {
        let exists = self.client.collection_exists(TAG_COLLECTION_NAME).await?;
        if !exists {
            self.client
                .create_collection(&CreateCollection {
                    collection_name: TAG_COLLECTION_NAME.to_string(),
                    vectors_config: Some(VectorsConfig {
                        config: Some(Config::ParamsMap(VectorParamsMap {
                            map: [(
                                "clip".to_string(),
                                VectorParams {
                                    size: STICKER_COLLECTION_SIZE,
                                    distance: STICKER_COLLECTION_DISTANCE.into(),
                                    ..Default::default()
                                },
                            )]
                            .into(),
                        })),
                    }),
                    ..Default::default()
                })
                .await?;
        }
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    async fn init_sticker_collection(&self) -> Result<(), VectorDatabaseError> {
        let exists = self
            .client
            .collection_exists(STICKER_COLLECTION_NAME)
            .await?;
        if !exists {
            self.client
                .create_collection(&CreateCollection {
                    collection_name: STICKER_COLLECTION_NAME.to_string(),
                    vectors_config: Some(VectorsConfig {
                        // config: Some(Config::Params(VectorParams {
                        //     size: STICKER_COLLECTION_SIZE,
                        //     distance: STICKER_COLLECTION_DISTANCE.into(),
                        //     ..Default::default()
                        // })),
                        config: Some(Config::ParamsMap(VectorParamsMap {
                            map: [
                                (
                                    "clip".to_string(),
                                    VectorParams {
                                        size: STICKER_COLLECTION_SIZE,
                                        distance: STICKER_COLLECTION_DISTANCE.into(),
                                        ..Default::default()
                                    },
                                ),
                                (
                                    "histogram".to_string(),
                                    VectorParams {
                                        size: 125,
                                        distance: Distance::Cosine.into(),
                                        datatype: Some(Datatype::Uint8.into()),
                                        ..Default::default()
                                    },
                                ),
                            ]
                            .into(),
                        })),
                    }),
                    ..Default::default()
                })
                .await?;
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, clip_vector), err(Debug))]
    pub async fn insert_tag(
        &self,
        clip_vector: Vec<f32>,
        tag_or_alias: String,
    ) -> Result<(), VectorDatabaseError> {
        let id = tag_to_uuid(&tag_or_alias);
        let payload: Payload = json!(
            {
                "tag_or_alias": tag_or_alias,
            }
        )
        .try_into()
        .expect("valid conversion");

        let points = vec![PointStruct::new(
            id,
            HashMap::from([("clip".to_string(), clip_vector)]),
            payload,
        )];
        self.client
            .upsert_points_blocking(TAG_COLLECTION_NAME, None, points, None)
            // .upsert_points_blocking(TAG_COLLECTION_NAME, None, points, None)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, clip_vector, histogram_vector), err(Debug))]
    pub async fn insert_sticker(
        &self,
        clip_vector: Vec<f32>,
        histogram_vector: Vec<u8>,
        file_hash: String,
    ) -> Result<(), VectorDatabaseError> {
        let id = file_hash_to_uuid(&file_hash);
        // TODO: add sticker_id + set_id + has_tags as payload
        let payload: Payload = json!(
            {
                "file_hash": file_hash,
                // "has_tags": has_tags,
            }
        )
        .try_into()
        .expect("valid conversion");

        let points = vec![PointStruct::new(
            id,
            HashMap::from([
                ("clip".to_string(), clip_vector),
                ("histogram".to_string(), vec_u8_to_f32(histogram_vector)),
            ]),
            payload,
        )];
        self.client
            .upsert_points_blocking(STICKER_COLLECTION_NAME, None, points, None)
            // .upsert_points_blocking(STICKER_COLLECTION_NAME, None, points, None)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn delete_stickers(&self, file_ids: Vec<String>) -> Result<(), VectorDatabaseError> {
        let ids = file_ids
            .into_iter()
            .map(|id| file_hash_to_uuid(&id).into())
            .collect_vec();
        let points = PointsIdsList { ids };
        self.client
            .delete_points(
                STICKER_COLLECTION_NAME,
                None,
                &qdrant_client::qdrant::PointsSelector {
                    points_selector_one_of: Some(
                        qdrant_client::qdrant::points_selector::PointsSelectorOneOf::Points(points),
                    ),
                },
                None,
            )
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn recommend_tags(
        &self,
        file_hash: &str,
    ) -> Result<Option<Vec<String>>, VectorDatabaseError> {
        let search_result = self
            .client
            .recommend(&RecommendPoints {
                collection_name: TAG_COLLECTION_NAME.into(),
                positive: vec![
                    // vector
                    file_hash_to_uuid(file_hash).into(),
                ],
                // lookup_from: Some(STICKER_COLLECTION_NAME),
                lookup_from: Some(LookupLocation {
                    collection_name: STICKER_COLLECTION_NAME.to_string(),
                    vector_name: Some("clip".to_string()),
                    ..Default::default()
                }),
                using: Some("clip".to_string()),
                // filter: Some(Filter::all([Condition::matches("bar", 12)])),
                limit: 20,
                with_payload: Some(true.into()), // TODO: only set payload to include sticker_id
                // with_payload: Some(vec!["file_hash"].into()),
                ..Default::default()
            })
            .await
            .convert_to_sensible_error()?;
        Ok(search_result.map(|search_result| convert_tag_recommend_response(search_result.result)))
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn recommend_tags_from_existing_tags(
        // TODO: use a different embedding model that was optimized just for text
        &self,
        tags: &[String],
    ) -> Result<Option<Vec<String>>, VectorDatabaseError> {
        let tags = tags
            .into_iter()
            .map(|tag| tag_to_uuid(&tag).into())
            .collect_vec();
        let search_result = self
            .client
            .recommend(&RecommendPoints {
                collection_name: TAG_COLLECTION_NAME.into(),
                positive: tags,
                using: Some("clip".to_string()),
                limit: 20,
                with_payload: Some(true.into()), // TODO: only set payload to include sticker_id
                strategy: Some(RecommendStrategy::BestScore.into()),
                // with_payload: Some(vec!["file_hash"].into()),
                ..Default::default()
            })
            .await
            .convert_to_sensible_error()?;
        Ok(search_result.map(|search_result| convert_tag_recommend_response(search_result.result)))
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn find_tags_given_vector(
        &self,
        clip_vector: Vec<f32>,
    ) -> Result<Vec<String>, VectorDatabaseError> {
        let search_result = self
            .client
            .search_points(&SearchPoints {
                collection_name: TAG_COLLECTION_NAME.into(),
                vector: clip_vector.into(),
                vector_name: Some("clip".to_string()),
                limit: 50,
                with_payload: Some(true.into()),
                ..Default::default()
            })
            .await?;
        Ok(convert_tag_recommend_response(search_result.result))
    }

    // #[tracing::instrument(skip(self), err(Debug))]
    // pub async fn scroll_stickers(
    //     &self,
    // ) -> Result<Vec<(Vec<f32>, Vec<u8>, String)>, VectorDatabaseError> {
    //     let result = self
    //         .client
    //         .scroll(&ScrollPoints {
    //             collection_name: STICKER_COLLECTION_NAME.into(),
    //             limit: Some(999999),
    //             with_payload: Some(true.into()),
    //             with_vectors: Some(true.into()),
    //             ..Default::default()
    //         })
    //         .await?;
    //     Ok(result
    //         .result
    //         .into_iter()
    //         .map(|scored_point| {
    //             (
    //                 scored_point
    //                     .vectors
    //                     .clone()
    //                     .map(|v| match v.vectors_options {
    //                         Some(options) => match options.clone() {
    //                             qdrant_client::qdrant::vectors::VectorsOptions::Vector(v) => {
    //                                 todo!()
    //                             }
    //                             qdrant_client::qdrant::vectors::VectorsOptions::Vectors(v) => {
    //                                 v.vectors["clip"].data.clone()
    //                             }
    //                         },
    //                         None => todo!(),
    //                     })
    //                     .unwrap(),
    //                 scored_point
    //                     .vectors
    //                     .map(|v| match v.vectors_options {
    //                         Some(options) => match options.clone() {
    //                             qdrant_client::qdrant::vectors::VectorsOptions::Vector(v) => {
    //                                 todo!()
    //                             }
    //                             qdrant_client::qdrant::vectors::VectorsOptions::Vectors(v) => v
    //                                 .vectors["histogram"]
    //                                 .data
    //                                 .clone()
    //                                 .into_iter()
    //                                 .map(|entry| entry.round().min(255.0) as u8)
    //                                 .collect_vec(),
    //                         },
    //                         None => todo!(),
    //                     })
    //                     .unwrap(),
    //                 scored_point.payload["file_hash"]
    //                     .as_str()
    //                     .unwrap()
    //                     .to_string(),
    //             )
    //         })
    //         .collect_vec())
    // }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn find_stickers_given_vector(
        &self,
        clip_vector: Vec<f32>,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<StickerMatch>, VectorDatabaseError> {
        let search_result = self
            .client
            .search_points(&SearchPoints {
                collection_name: STICKER_COLLECTION_NAME.into(),
                vector: clip_vector.into(),
                vector_name: Some("clip".to_string()),
                limit,
                offset: Some(offset),
                with_payload: Some(true.into()),
                ..Default::default()
            })
            .await?;
        Ok(convert_sticker_recommend_response(search_result.result))
    }

    /// returns file hashes
    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn find_similar_stickers(
        &self,
        positive_file_ids: &[String],
        negative_file_ids: &[String],
        similarity_aspect: SimilarityAspect,
        score_threshold: f32,
        limit: u64,
        offset: u64,
        // vector: Vec<f32>
    ) -> Result<Option<Vec<StickerMatch>>, VectorDatabaseError> {
        let vector_name = match similarity_aspect {
            SimilarityAspect::Color => "histogram".to_string(),
            SimilarityAspect::Embedding => "clip".to_string(),
        };
        let strategy = if positive_file_ids.len() != 1 || negative_file_ids.len() != 0 {
            Some(RecommendStrategy::BestScore.into())
        } else {
            None
        };
        // TODO: create sticker result struct
        let search_result = self
            .client
            .recommend(&RecommendPoints {
                collection_name: STICKER_COLLECTION_NAME.into(),
                positive: positive_file_ids
                    .into_iter()
                    .map(|id| file_hash_to_uuid(id).into())
                    .collect_vec(),
                negative: negative_file_ids
                    .into_iter()
                    .map(|id| file_hash_to_uuid(id).into())
                    .collect_vec(),
                using: Some(vector_name), // TODO: allow to switch between clip and historgram
                // filter: Some(Filter::all([Condition::matches("bar", 12)])),
                limit,
                offset: Some(offset),
                with_payload: Some(true.into()), // TODO: only set payload to include sticker_id
                score_threshold: Some(score_threshold),
                strategy,
                // with_payload: Some(vec!["file_hash"].into()),
                ..Default::default()
            })
            .await
            .convert_to_sensible_error()?;
        Ok(search_result
            .map(|search_result| convert_sticker_recommend_response(search_result.result)))
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn compare_sticker_similarities(
        &self,
        file_hash_a: String,
        file_hash_b: String,
    ) -> Result<StickerSimilarities, VectorDatabaseError> {
        let a = file_hash_to_uuid(&file_hash_a);
        let b = file_hash_to_uuid(&file_hash_b);

        let points = self
            .client
            .get_points(
                STICKER_COLLECTION_NAME,
                None,
                &[a.into(), b.into()],
                Some(true),
                Some(false),
                None,
            )
            .await?;
        let vectors = points
            .result
            .into_iter()
            .filter_map(|point| {
                point.vectors.and_then(|v| {
                    v.vectors_options.and_then(|o| match o {
                        qdrant_client::qdrant::vectors_output::VectorsOptions::Vector(_) => None,
                        qdrant_client::qdrant::vectors_output::VectorsOptions::Vectors(v) => {
                            Some(v.vectors)
                        }
                    })
                })
            })
            .collect_vec();

        let a = vectors
            .get(0)
            .ok_or_else(|| VectorDatabaseError::MissingVector("0".to_string()))?;
        let b = vectors
            .get(1)
            .ok_or_else(|| VectorDatabaseError::MissingVector("1".to_string()))?;

        let clip_a = a
            .get("clip")
            .ok_or_else(|| VectorDatabaseError::MissingVector("clip".to_string()))?;
        let clip_b = b
            .get("clip")
            .ok_or_else(|| VectorDatabaseError::MissingVector("clip".to_string()))?;
        let hist_a = a
            .get("histogram")
            .ok_or_else(|| VectorDatabaseError::MissingVector("histogram".to_string()))?;
        let hist_b = b
            .get("histogram")
            .ok_or_else(|| VectorDatabaseError::MissingVector("histogram".to_string()))?;

        Ok(StickerSimilarities {
            clip: cosine_similarity(clip_a.data.clone(), clip_b.data.clone()),
            histogram: cosine_similarity(hist_a.data.clone(), hist_b.data.clone()),
        })
    }

    #[tracing::instrument(skip(self), err(Debug))]
    pub async fn find_missing_stickers(
        &self,
        file_hashes: Vec<String>,
    ) -> Result<Vec<String>, VectorDatabaseError> {
        let point_uuids = file_hashes
            .iter()
            .map(|hash| file_hash_to_uuid(&hash).into())
            .collect_vec();
        let search_result = self
            .client
            .get_points(
                STICKER_COLLECTION_NAME,
                None,
                point_uuids.as_slice(),
                Some(false),
                Some(false),
                None,
            )
            .await?;
        let zipped = file_hashes
            .into_iter()
            .zip(point_uuids.into_iter())
            .collect_vec();
        let points = search_result
            .result
            .into_iter()
            .flat_map(|point| point.id)
            .collect_vec();

        let missing = zipped
            .into_iter()
            .filter(|(hash, uuid)| !points.contains(uuid))
            .map(|(hash, _)| hash)
            .collect_vec();
        // TODO: test the "missing" logic in an unit test

        Ok(missing)
        // convert_tag_recommend_response(search_result.result)
    }
}

pub struct StickerSimilarities {
    pub clip: f32,
    pub histogram: f32,
}

#[derive(Debug, Clone)]
pub struct StickerMatch {
    pub file_hash: String,
    pub score: f32,
}

fn convert_tag_recommend_response(scored_points: Vec<ScoredPoint>) -> Vec<String> {
    scored_points
        .into_iter()
        .map(|scored_point| {
            scored_point
                .payload
                .get("tag_or_alias")
                .map(|val| val.as_str().map(|str| str.to_string()))
        })
        .filter_map(|v| v)
        .filter_map(|v| v)
        .collect_vec()
}

fn convert_sticker_recommend_response(scored_points: Vec<ScoredPoint>) -> Vec<StickerMatch> {
    scored_points
        .into_iter()
        .map(|scored_point| {
            scored_point.payload.get("file_hash").map(|val| {
                val.as_str().map(|str| StickerMatch {
                    file_hash: str.to_string(),
                    score: scored_point.score,
                })
            })
        })
        .filter_map(|v| v)
        .filter_map(|v| v)
        .collect_vec()
}

fn file_hash_to_uuid(file_hash: &str) -> String {
    create_uuid_v5(&format!("fuzzle:sticker-file:{file_hash}"))
}

fn tag_to_uuid(tag: &str) -> String {
    create_uuid_v5(&format!("fuzzle:tag:{tag}"))
}

fn create_uuid_v5(url: &str) -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, url.as_bytes())
        .hyphenated()
        .encode_lower(&mut Uuid::encode_buffer())
        .to_string()
}
