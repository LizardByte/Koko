#![doc = "Authentication utilities for the application."]

// lib imports
use base64::{engine::general_purpose, Engine as _};
use bcrypt::{hash, verify, DEFAULT_COST};
use diesel::{QueryDsl, RunQueryDsl};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use once_cell::sync::Lazy;
use rand::Rng;
use rocket::http::Status;
use rocket::outcome::Outcome;
use rocket::request::{self, FromRequest, Request};
use rocket_okapi::request::{OpenApiFromRequest, RequestHeaderInput};
use serde::{Deserialize, Serialize};

// local imports
use crate::db::DbConn;

/// Guard for admin routes.
pub struct AdminGuard(Claims);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AdminGuard {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let claims = match request.guard::<Claims>().await {
            Outcome::Success(claims) => claims,
            _ => return Outcome::Error((Status::Unauthorized, ())),
        };

        let db = match request.guard::<DbConn>().await {
            Outcome::Success(db) => db,
            _ => return Outcome::Error((Status::InternalServerError, ())),
        };

        let user_id: i32 = match claims.sub.parse() {
            Ok(id) => id,
            Err(_) => return Outcome::Error((Status::Unauthorized, ())),
        };

        let is_admin = db
            .run(move |conn| {
                use crate::db::schema::users::dsl::*;
                users.find(user_id).select(admin).first::<bool>(conn)
            })
            .await
            .unwrap_or(false);

        if is_admin {
            Outcome::Success(AdminGuard(claims))
        } else {
            Outcome::Error((Status::Forbidden, ()))
        }
    }
}

#[rocket::async_trait]
impl OpenApiFromRequest<'_> for AdminGuard {
    fn from_request_input(
        _gen: &mut rocket_okapi::gen::OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        Ok(RequestHeaderInput::None)
    }
}

/// Claims for the JWT.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub(crate) sub: String,
    exp: usize,
}

const BEARER: &str = "Bearer ";

/// Create a JWT token.
pub fn create_token(
    user_id: &str,
    secret: &str,
) -> String {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::seconds(60))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        sub: user_id.to_owned(),
        exp: expiration as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
    .unwrap()
}

/// Decode a JWT token.
pub fn decode_token(
    token: &str,
    secret: &str,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )
    .map(|data| data.claims)
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Claims {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let keys: Vec<_> = request.headers().get("Authorization").collect();
        if keys.len() != 1 {
            return Outcome::Error((rocket::http::Status::Unauthorized, ()));
        }

        if !keys[0].starts_with(BEARER) {
            return Outcome::Error((rocket::http::Status::Unauthorized, ()));
        }

        let token = &keys[0][BEARER.len()..];
        let secret = get_jwt_secret();

        match decode_token(token, secret) {
            Ok(claims) => Outcome::Success(claims),
            Err(_) => Outcome::Error((rocket::http::Status::Unauthorized, ())),
        }
    }
}

impl OpenApiFromRequest<'_> for Claims {
    fn from_request_input(
        _gen: &mut rocket_okapi::gen::OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        Ok(RequestHeaderInput::None)
    }
}

static JWT_SECRET: Lazy<String> = Lazy::new(|| {
    let random_bytes: [u8; 32] = rand::rng().random();
    general_purpose::STANDARD.encode(random_bytes)
});

pub(crate) fn get_jwt_secret() -> &'static str {
    &JWT_SECRET
}

pub(crate) fn hash_password(password: &str) -> String {
    hash(password, DEFAULT_COST).unwrap()
}

pub(crate) fn verify_password(
    password: &str,
    hash: &str,
) -> bool {
    verify(password, hash).unwrap_or(false)
}
