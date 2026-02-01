use std::sync::Arc;

use reqwest::{Client, Error, Response};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use url::Url;

use crate::fmetrics::TracedRequest;

struct Inner {
    http_client: Client,
    base_url: Url,
}

#[derive(Clone)]
pub struct ExternalTelegramService {
    inner: Arc<Inner>,
}

/// Telegram features handled by an external service
impl ExternalTelegramService {
    pub fn new(base_url: &str) -> Self {
        Self {
            inner: Arc::new(Inner {
                http_client: Client::new(),
                base_url: Url::parse(&base_url)
                    .expect("invalid base url for external telegram services"),
            }),
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_sticker_pack_id(&self, name: String) -> Option<StickerPackResponse> {
        let py_result = self
            .inner
            .http_client
            .post(self.inner.base_url.join("/stickers/resolve").unwrap())
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "short_name": name }))
            .send_traced("HTTP POST /stickers/resolve")
            .await;
        Self::handle_res(py_result).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn list_discovered_sticker_packs(
        &self,
        offset: usize,
        limit: usize,
    ) -> Option<StickerPacksResponse> {
        let py_result = self
            .inner
            .http_client
            .get(self.inner.base_url.join("/stickers/packs").unwrap())
            .query(&[("offset", offset.to_string()), ("limit", limit.to_string())])
            .send_traced("HTTP GET /stickers/packs")
            .await;
        Self::handle_res(py_result).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn resolve_user_id_to_username(&self, user_id: i64) -> Option<UserResolveResponse> {
        let py_result = self
            .inner
            .http_client
            .post(self.inner.base_url.join("/user/resolve").unwrap())
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "user_id": user_id }))
            .send_traced("HTTP POST /user/resolve")
            .await;
        Self::handle_res(py_result).await
    }

    #[tracing::instrument(skip(self))]
    pub async fn resolve_username_to_id(&self, name: String) -> Option<EntityResolveResponse> {
        let py_result = self
            .inner
            .http_client
            .post(self.inner.base_url.join("/entity/resolve").unwrap())
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({ "username": name }))
            .send_traced("HTTP POST /entity/resolve")
            .await;
        Self::handle_res(py_result).await
    }

    async fn handle_res<T: DeserializeOwned>(py_result: Result<Response, Error>) -> Option<T> {
        match py_result {
            Ok(resp) => match resp.error_for_status() {
                Ok(success_resp) => match success_resp.json::<T>().await {
                    Ok(py_response) => Some(py_response),
                    Err(e) => {
                        tracing::warn!(error = %e, "Invalid external telegram service response");
                        None
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "External telegram service returned error status");
                    None
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "Failed to call External telegram service");
                None
            }
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct StickerPackResponse {
    pub short_name: String,
    pub title: String,
    pub telegram_pack_id: i64,
}

#[derive(Deserialize, Debug)]
pub struct StickerPacksResponse {
    pub packs: Vec<StickerPackResponseEntry>,
}

#[derive(Deserialize, Debug)]
pub struct StickerPackResponseEntry {
    pub short_name: String,
    pub title: String,
    pub telegram_pack_id: Option<i64>,
}

#[derive(Deserialize, Debug)]
pub struct UserResolveResponse {
    pub username: String,
}

#[derive(Deserialize, Debug)]
pub struct EntityResolveResponse {
    pub canonical_username: String,
    pub entity_id: i64,
    pub is_user: bool,
}
