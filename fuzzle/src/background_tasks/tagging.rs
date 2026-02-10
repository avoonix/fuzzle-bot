use chrono::{DateTime, Utc};
use itertools::Itertools;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, PoisonError, RwLock,
    },
};
use tracing::warn;

use tokio::sync::{mpsc, oneshot};

use crate::{
    bot::InternalError,
    database::Database,
    tags::{ScoredTagSuggestion, Tfidf},
    util::Emoji,
};

use super::TagManagerService;

#[derive(Clone)]
pub struct TfIdfService {
    tag_manager: TagManagerService,
    database: Database,
    last_computed: Arc<RwLock<DateTime<Utc>>>,
    tfidf: Arc<RwLock<Tfidf<String, Emoji>>>,
    inverse_tfidf: Arc<RwLock<Tfidf<Emoji, String>>>,
    is_computing: Arc<AtomicBool>,
}

impl TfIdfService {
    #[tracing::instrument(skip(database, tag_manager))]
    pub async fn new(
        database: Database,
        tag_manager: TagManagerService,
    ) -> Result<Self, InternalError> {
        let all_used_tags: Vec<(Emoji, String, i64)> = database.get_all_tag_emoji_pairs().await?;
        let inverse = all_used_tags
            .iter()
            .map(|(a, b, c)| (b.clone(), a.clone(), *c))
            .collect_vec();

        let tfidf_span =
            tracing::info_span!("spawn_blocking_tfidf").or_current();
        let tfidf = tokio::task::spawn_blocking(move || {
            tfidf_span.in_scope(|| {
                Arc::new(RwLock::new(Tfidf::generate(all_used_tags)))
            })
        });
        let inverse_tfidf_span =
            tracing::info_span!("spawn_blocking_inverse_tfidf").or_current();
        let inverse_tfidf =
            tokio::task::spawn_blocking(move || inverse_tfidf_span.in_scope(|| Arc::new(RwLock::new(Tfidf::generate(inverse)))));
        let (tfidf, inverse_tfidf) = tokio::try_join!(tfidf, inverse_tfidf)?;

        let last_computed = Arc::new(RwLock::new(chrono::Utc::now()));
        let service = Self {
            tag_manager,
            database,
            last_computed,
            tfidf,
            inverse_tfidf,
            is_computing: Arc::new(false.into()),
        };
        Ok(service)
    }

    /// does not wait for completion
    pub async fn request_recompute(&self) {
        let should_recompute = {
            chrono::Utc::now() - *self.last_computed.read().unwrap() > chrono::Duration::minutes(5)
        };
        if !should_recompute {
            return;
        }
        if self
            .is_computing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            == Ok(false)
        {
            let val = self.clone();
            tokio::spawn(async move {
                let all_used_tags = val.database.get_all_tag_emoji_pairs().await;
                let all_used_tags = match all_used_tags {
                    Ok(t) => t,
                    Err(err) => {
                        tracing::error!("async task error: {err:?}");
                        return;
                    }
                };

                // TODO: also recompute inverse
                let tfidf_span =
                    tracing::info_span!("spawn_blocking_regenerate_tfidf").or_current();
                let res = tokio::task::spawn_blocking(move || tfidf_span.in_scope(|| Tfidf::generate(all_used_tags)))
                    .await
                    .unwrap();
                {
                    let mut t = val.tfidf.write().unwrap();
                    *t = res;
                }
                {
                    let mut last_computed = val.last_computed.write().unwrap();
                    *last_computed = chrono::Utc::now();
                }
                val.is_computing.store(false, Ordering::Release);
            });
        }
    }

    pub async fn suggest_tags_for_sticker(
        &self,
        sticker_id: &str,
    ) -> Result<Vec<ScoredTagSuggestion>, InternalError> {
        Ok(self
            .database
            .get_sticker_by_id(sticker_id)
            .await?
            .and_then(|sticker| {
                sticker
                    .emoji
                    .map(|e| Emoji::new_from_string_single(e))
                    .map(|emoji| self.tfidf.read().unwrap().suggest(emoji))
            })
            .unwrap_or_default())
    }

    pub async fn suggest_emojis_for_tag(
        &self,
        tag: String,
    ) -> Result<Vec<ScoredTagSuggestion<Emoji>>, InternalError> {
        Ok(self.inverse_tfidf.read().unwrap().suggest(tag))
    }

    pub async fn suggest_tags_for_emoji(
        &self,
        emoji: Emoji,
    ) -> Result<Vec<ScoredTagSuggestion>, InternalError> {
        Ok(self.tfidf.read().unwrap().suggest(emoji))
    }
}
