use itertools::Itertools;
use leptos::*;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, str::FromStr, vec};

#[cfg(feature = "ssr")]
pub mod ssr {
    pub use crate::inline::query_stickers;
    pub use crate::inline::InlineQueryData;
    pub use crate::web::server::AppState;
    pub use crate::web::server::AuthenticatedUser;
    pub use actix_web::web;
    pub use actix_web::web::{Data, Query};
    pub use actix_web::HttpRequest;
    pub use leptos::ServerFnError;
    pub use leptos_actix::extract;
}

#[server]
pub async fn fetch_results(
    query: String,
    limit: usize,
    offset: usize,
) -> Result<Vec<StickerDto>, ServerFnError> {
    use self::ssr::*;
    let (user, data): (AuthenticatedUser, Data<AppState>) = extract().await?;

    let query =
        InlineQueryData::try_from(query).map_err(|err| ServerFnError::new("invalid query"))?;

    let result = match query.mode.clone() {
        crate::inline::InlineQueryDataMode::StickerSearch { emoji } => {
            let result = query_stickers(
                query,
                data.database.clone(),
                emoji,
                user.user_meta,
                data.tag_manager.clone(),
                limit,
                offset,
            )
            .await
            .map_err(|err| ServerFnError::new("bot error"))?; // TODO: limit, offset, proper error handling
            result
        }
        _ => Err(ServerFnError::new("query not implemented"))?,
    };

    Ok(result
        .into_iter()
        .map(|s| StickerDto { id: s.id })
        .collect_vec())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StickerDto {
    pub id: String,
}
