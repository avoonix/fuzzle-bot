use crate::{bot::{InternalError, get_or_create_user}, database::User, util::Required, web::shared::AppState};
use actix_web::{
    error::{ErrorInternalServerError, ErrorUnauthorized},
    web, FromRequest, HttpRequest,
};
use chrono::{naive::serde::ts_seconds, TimeDelta};
use futures::Future;
use itertools::Itertools;
use ring::{digest, hmac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    pin::Pin,
    sync::Arc,
};
use teloxide::types::UserId;

pub const AUTH_COOKIE_NAME: &str = "fuzzlebot_login_data";

// adapted from https://docs.rs/telegram-login/latest/src/telegram_login/lib.rs.html
#[derive(Serialize, Deserialize, Debug)]
pub struct AuthData {
    pub id: u64,
    first_name: Option<String>,
    last_name: Option<String>,
    pub username: Option<String>,
    photo_url: Option<String>,
    #[serde(with = "ts_seconds")]
    auth_date: chrono::NaiveDateTime,
    hash: Option<String>,
}

#[derive(Deserialize)]
pub struct AuthDataWebApp {
    id: u64,
    first_name: Option<String>,
    last_name: Option<String>,
    username: Option<String>,
}

impl AuthDataWebApp {
    fn as_unhashed_auth_data(self, auth_date: chrono::NaiveDateTime) -> AuthData {
        AuthData {
            id: self.id,
            first_name: self.first_name,
            last_name: self.last_name,
            username: self.username,
            auth_date,
            photo_url: None,
            hash: None,
        }
    }
}

impl AuthData {
    #[tracing::instrument(skip(self, bot_token))]
    pub fn check(&self, bot_token: String) -> bool {
        match self.hash {
            Some(ref hash) => match hex::decode(hash) {
                Ok(hash) => {
                    // TODO: does not check age yet
                    let data_check_string = self.data_check_string();
                    let secret_key = digest::digest(&digest::SHA256, bot_token.as_bytes());
                    let v_key = hmac::Key::new(hmac::HMAC_SHA256, secret_key.as_ref());
                    hmac::verify(&v_key, data_check_string.as_bytes(), &hash).is_ok()
                }
                Err(_e) => false,
            },
            None => false,
        }
    }

    fn data_check_string(&self) -> String {
        let auth_date = self.auth_date.and_utc().timestamp().to_string();
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
pub struct OptionalAuthenticatedUser {
    pub auth_data: Option<Arc<AuthData>>,
}

impl FromRequest for OptionalAuthenticatedUser {
    type Error = actix_web::Error; // TODO: different error type?
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut actix_web::dev::Payload) -> Self::Future {
        let data = req
            .app_data::<web::Data<AppState>>()
            .expect("data to be present");
        let data = data.clone();
        let cookie = req.cookie(AUTH_COOKIE_NAME);
        Box::pin(async move {
            let Some(cookie) = cookie else {
                return Ok(Self {auth_data: None})
            };
            let auth_data: AuthData = serde_json::from_str(cookie.value())?;
            let is_ok = auth_data.check(data.config.telegram_bot_token.clone());
            if is_ok {
                Ok(Self {
                    auth_data: Some(Arc::new(auth_data)),
                })
            } else {
                Ok(Self {auth_data: None})
            }
        })
    }
}

#[derive(Debug)]
pub struct AuthenticatedUser {
    pub auth_data: Arc<AuthData>,
}

impl FromRequest for AuthenticatedUser {
    type Error = actix_web::Error; // TODO: different error type?
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut actix_web::dev::Payload) -> Self::Future {
        let res = OptionalAuthenticatedUser::from_request(req, payload);
        Box::pin(async move {
            let res = res.await?;
            match res.auth_data {
                Some(auth_data) => Ok(Self { auth_data }),
                None => Err(ErrorUnauthorized("unauthorized"))
            }
        })
    }
}

// TODO: clone is probably not required?
#[derive(Debug, Deserialize, Clone)]
pub struct WebAppInitData {
    hash: String,
    #[serde(flatten)]
    other: HashMap<String, String>,
}

impl WebAppInitData {
    #[tracing::instrument(skip(self, bot_token))]
    pub fn check(&self, bot_token: String) -> bool {
        match hex::decode(&self.hash) {
            Ok(hash) => {
                let data_check_string = self.data_check_string();

                // Create secret key as HMAC-SHA256(bot_token, "WebAppData")
                let key = hmac::Key::new(hmac::HMAC_SHA256, "WebAppData".as_bytes());
                let secret_key = hmac::sign(&key, bot_token.as_bytes());

                // Use secret key to verify the data
                let v_key = hmac::Key::new(hmac::HMAC_SHA256, secret_key.as_ref());
                hmac::verify(&v_key, data_check_string.as_bytes(), &hash).is_ok()
            }
            Err(_) => false,
        }
    }

    fn data_check_string(&self) -> String {
        self.other
            .iter()
            .sorted_by_key(|(key, _)| key.to_string())
            .map(|(name, value)| format!("{name}={value}"))
            .join("\n")
    }

    pub fn into_auth_data(self, bot_token: String) -> Result<AuthData, InternalError> {
        let user = self.other.get("user").required()?;
        let user: AuthDataWebApp = serde_json::from_str(&user)?;
        let mut user = user.as_unhashed_auth_data(chrono::Utc::now().naive_utc());
        let data_check_string = user.data_check_string();
        let secret_key = digest::digest(&digest::SHA256, bot_token.as_bytes());
        let v_key = hmac::Key::new(hmac::HMAC_SHA256, secret_key.as_ref());
        user.hash = Some(hex::encode(hmac::sign(
            &v_key,
            data_check_string.as_bytes(),
        )));
        Ok(user)
    }
}
