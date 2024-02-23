use super::AppState;
use crate::bot::{get_or_create_user, UserMeta};
use actix_web::{
    error::{ErrorInternalServerError, ErrorUnauthorized},
    web, FromRequest, HttpRequest,
};
use chrono::naive::serde::ts_seconds;
use futures::Future;
use itertools::Itertools;
use ring::{digest, hmac};
use serde::{Deserialize, Serialize};
use std::{pin::Pin, sync::Arc};
use teloxide::types::UserId;

pub const AUTH_COOKIE_NAME: &str = "fuzzlebot_login_data";

// adapted from https://docs.rs/telegram-login/latest/src/telegram_login/lib.rs.html
#[derive(Serialize, Deserialize, Debug)]
pub struct AuthData {
    id: u64,
    first_name: Option<String>,
    last_name: Option<String>,
    pub username: Option<String>,
    photo_url: Option<String>,
    #[serde(with = "ts_seconds")]
    auth_date: chrono::NaiveDateTime,
    hash: String,
}

impl AuthData {
    pub fn check(&self, bot_token: String) -> bool {
        match hex::decode(&self.hash) {
            Ok(hash) => {
                // TODO: does not check age yet
                let data_check_string = self.data_check_string();
                let secret_key = digest::digest(&digest::SHA256, &bot_token.as_bytes());
                let v_key = hmac::Key::new(hmac::HMAC_SHA256, secret_key.as_ref());
                hmac::verify(&v_key, data_check_string.as_bytes(), &hash).is_ok()
            }
            Err(_e) => false,
        }
    }

    fn data_check_string(&self) -> String {
        let auth_date = self.auth_date.timestamp().to_string();
        let id = self.id.to_string();
        let fields = vec![
            // sorted alphabetically
            ("auth_date", Some(&auth_date)),
            ("first_name", self.first_name.as_ref()),
            ("id", Some(&id)),
            ("last_name", self.last_name.as_ref()),
            ("photo_url", self.photo_url.as_ref()),
            ("username", self.username.as_ref()),
        ];
        fields
            .into_iter()
            .filter_map(|(name, value)| value.map(|value| format!("{name}={value}")))
            .join("\n")
    }
}

#[derive(Debug)]
pub struct AuthenticatedUser {
    pub auth_data: Arc<AuthData>,
    pub user_meta: Arc<UserMeta>,
}

impl FromRequest for AuthenticatedUser {
    type Error = actix_web::Error; // TODO: different error type?
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut actix_web::dev::Payload) -> Self::Future {
        let data = req
            .app_data::<web::Data<AppState>>()
            .expect("data to be present");
        let data = data.clone(); // TODO: fixes lifetime warning, but is probably not a good fix

        let cookie = req.cookie(AUTH_COOKIE_NAME);
        Box::pin(async move {
            let cookie = cookie.ok_or(ErrorUnauthorized("cookie missing"))?;
            let auth_data: AuthData = serde_json::from_str(cookie.value())?;
            let is_ok = auth_data.check(data.config.telegram.token.clone());
            if is_ok {
                let user_meta = get_or_create_user(
                    UserId(auth_data.id),
                    data.config.clone(),
                    data.database.clone(),
                    data.bot.clone(),
                )
                .await
                .map_err(|err| ErrorInternalServerError("failed to create/get user"))?; // TODO: properly handle

                Ok(AuthenticatedUser {
                    auth_data: auth_data.into(),
                    user_meta: user_meta.into(),
                })
            } else {
                Err(ErrorUnauthorized("invalid auth data")) // TODO: properly handle
            }
        })
    }
}
