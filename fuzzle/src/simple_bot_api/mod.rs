// a simple alternative until teloxide supports the new api

use itertools::Itertools;
use serde_json::{json, Map, Value};
use teloxide::types::{StickerFormat, UserId};

use crate::bot::{BotError, InternalError};

async fn perform_request(method: &str, token: &str, body: Value) -> Result<(), InternalError> {
    let client = reqwest::Client::new();
    let res: Map<String, Value> = client
        .post(format!("https://api.telegram.org/bot{token}/{method}"))
        .json(&body)
        .send()
        .await?
        .json()
        .await?;

    if res.get("ok") != Some(&Value::Bool(true)) {
        tracing::error!("error during {method}: {res:?}");
        let description = res.get("description").unwrap_or(&Value::Null).as_str();
        if let Some(description) = description {
            Err(InternalError::Other(anyhow::anyhow!(description.to_string())).into())
        } else {
            Err(InternalError::Other(anyhow::anyhow!("error during {method}")).into())
        }
    } else {
        Ok(())
    }
}

/// 0-120 characters
#[tracing::instrument(skip(token))]
pub async fn set_my_short_description(
    token: &str,
    short_description: &str,
) -> Result<(), InternalError> {
    perform_request(
        "setMyShortDescription",
        token,
        json!({
            "short_description": short_description,
        }),
    ).await
}

/// 0-512 characters, plain text
#[tracing::instrument(skip(token))]
pub async fn set_my_description(
    token: &str,
    description: &str,
) -> Result<(), InternalError> {
    perform_request(
        "setMyDescription",
        token,
        json!({
            "description": description,
        }),
    ).await
}

#[tracing::instrument(skip(token))]
pub async fn create_new_sticker_set(
    token: &str,
    user_id: UserId,
    set_id: &str,
    title: &str,
    sticker_file_id: &str,
    format: &str,
    emoji_list: &[String],
    keywords: &[String],
) -> Result<(), InternalError> {
    perform_request(
        "createNewStickerSet",
        token,
        json!({
            "user_id": user_id.0,
            "name": set_id,
            "title": title,
            "stickers": [{
                "sticker": sticker_file_id,
                "format": format,
                "emoji_list": emoji_list,
                "keywords": limit_keywords(keywords),
            }]
        }),
    ).await
}

/// https://core.telegram.org/bots/api#inputsticker
#[tracing::instrument(skip(token))]
pub async fn add_sticker_to_set(
    token: &str,
    user_id: UserId,
    set_id: &str,
    sticker_file_id: &str,
    format: &str,
    emoji_list: &[String],
    keywords: &[String],
) -> Result<(), InternalError> {
    perform_request(
        "addStickerToSet",
        token,
        json!({
            "user_id": user_id.0,
            "name": set_id,
            "sticker": {
                "sticker": sticker_file_id,
                "format": format,
                "emoji_list": emoji_list,
                "keywords": limit_keywords(keywords),
            }
        }),
    ).await
}

fn limit_keywords(keywords: &[String]) -> Vec<&String> {
    let mut len = 0;
    keywords.iter().take_while(|keyword| {
        len += keyword.len();
        len < 64 // maximum combined keyword length
    }).collect_vec()
}

// #[tracing::instrument(skip(token))]
// pub async fn set_sticker_keywords(token: &str, sticker_file_id: &str, keywords: &[String]) {
//     perform_request(
//         "setStickerKeywords",
//         token,
//         json!({
//             "sticker": sticker_file_id,
//             "keywords": keywords,
//         }),
//     );
// }
