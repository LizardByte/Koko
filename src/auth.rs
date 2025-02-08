#![doc = "Authentication utilities for the application."]

// lib imports
use base64::{engine::general_purpose, Engine as _};
use bcrypt::{hash, DEFAULT_COST};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use once_cell::sync::Lazy;
use rand::distr::Alphanumeric;
use rand::{rng, Rng};
use rocket::outcome::Outcome;
use rocket::request::{self, FromRequest, Request};
use rocket_okapi::request::{OpenApiFromRequest, RequestHeaderInput};
use serde::{Deserialize, Serialize};

/// Claims for the JWT.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    sub: String,
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

fn get_jwt_secret() -> &'static str {
    &JWT_SECRET
}

pub(crate) fn generate_salt() -> String {
    rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect()
}

pub(crate) fn hash_with_salt(
    salt: String,
    string: &str,
) -> String {
    let salted_input = format!("{}{}", salt, string);
    hash(salted_input, DEFAULT_COST).unwrap()
}
