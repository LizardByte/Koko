//! Authentication utilities for the application.

// lib imports
use base64::{
    Engine as _,
    engine::general_purpose,
};
use bcrypt::{
    DEFAULT_COST,
    hash,
    verify,
};
use diesel::{
    OptionalExtension,
    QueryDsl,
    RunQueryDsl,
};
use jsonwebtoken::{
    Algorithm,
    DecodingKey,
    EncodingKey,
    Header,
    Validation,
    decode,
    encode,
};
use once_cell::sync::Lazy;
use rand::RngExt;
use rocket::http::{
    Cookie,
    CookieJar,
    SameSite,
    Status,
};
use rocket::outcome::Outcome;
use rocket::request::{
    self,
    FromRequest,
    Request,
};
use rocket_okapi::request::{
    OpenApiFromRequest,
    RequestHeaderInput,
};
use serde::{
    Deserialize,
    Serialize,
};

// local imports
use crate::db::DbConn;

/// Enum defining different authorization roles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Any authenticated user
    User,
    /// Administrator role
    Admin,
}

impl Role {
    /// Get the scope string for OpenAPI documentation
    fn scope(&self) -> Vec<String> {
        match self {
            Role::User => vec![],
            Role::Admin => vec!["admin".to_owned()],
        }
    }
}

/// Generic authorization guard that can handle different roles
///
/// Place this before `DbConn` in route arguments so its account lookup releases
/// the connection before the handler acquires its own.
pub struct AuthGuard<const ROLE: u8> {
    claims: Claims,
}

impl<const ROLE: u8> AuthGuard<ROLE> {
    /// Get the claims contained in this guard
    pub fn claims(&self) -> &Claims {
        &self.claims
    }

    /// Get the role for this guard
    pub fn role() -> Role {
        match ROLE {
            0 => Role::User,
            1 => Role::Admin,
            _ => panic!("Invalid role constant"),
        }
    }
}

// Type aliases for convenience
/// Guard for any authenticated user
pub type UserGuard = AuthGuard<0>;
/// Guard for admin users only
pub type AdminGuard = AuthGuard<1>;

#[rocket::async_trait]
impl<'r, const ROLE: u8> FromRequest<'r> for AuthGuard<ROLE> {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let claims = match request.guard::<Claims>().await {
            Outcome::Success(claims) => claims,
            _ => return Outcome::Error((Status::Unauthorized, ())),
        };

        let role = Self::role();

        // Resolve every token against the current account, including ordinary users.
        let db = match request.guard::<DbConn>().await {
            Outcome::Success(db) => db,
            _ => return Outcome::Error((Status::InternalServerError, ())),
        };

        let user_id: i32 = match claims.sub.parse() {
            Ok(id) => id,
            Err(_) => return Outcome::Error((Status::Unauthorized, ())),
        };

        let account = db
            .run(move |conn| {
                use crate::db::schema::users::dsl::{
                    admin,
                    users,
                };
                users
                    .find(user_id)
                    .select(admin)
                    .first::<bool>(conn)
                    .optional()
            })
            .await;
        let is_admin = match account {
            Ok(Some(is_admin)) => is_admin,
            Ok(None) => return Outcome::Error((Status::Unauthorized, ())),
            Err(_) => return Outcome::Error((Status::InternalServerError, ())),
        };

        if role == Role::User || is_admin {
            // Restore native media authentication for clients with an existing bearer token.
            if request.uri().path() == "/api/v1/bootstrap" {
                if let Some(token) = request
                    .headers()
                    .get_one("Authorization")
                    .and_then(|header| header.strip_prefix(BEARER))
                {
                    set_media_auth_cookie(request.cookies(), token);
                }
            }
            Outcome::Success(AuthGuard { claims })
        } else {
            Outcome::Error((Status::Forbidden, ()))
        }
    }
}

/// Helper function to create Bearer token security configuration for OpenAPI
fn create_bearer_auth_security(scopes: Vec<String>) -> rocket_okapi::Result<RequestHeaderInput> {
    use rocket_okapi::okapi::Map;
    use rocket_okapi::okapi::openapi3::{
        SecurityRequirement,
        SecurityScheme,
        SecuritySchemeData,
    };

    let security_scheme = SecurityScheme {
        data: SecuritySchemeData::Http {
            scheme: "bearer".to_string(),
            bearer_format: Some("JWT".to_string()),
        },
        description: Some("JWT Bearer token authentication".to_string()),
        extensions: Map::new(),
    };

    let mut security_req = SecurityRequirement::new();
    security_req.insert("BearerAuth".to_owned(), scopes);

    Ok(RequestHeaderInput::Security(
        "BearerAuth".to_owned(),
        security_scheme,
        security_req,
    ))
}

impl<const ROLE: u8> OpenApiFromRequest<'_> for AuthGuard<ROLE> {
    fn from_request_input(
        _gen: &mut rocket_okapi::gen::OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> rocket_okapi::Result<RequestHeaderInput> {
        let role = Self::role();
        create_bearer_auth_security(role.scope())
    }
}

/// Claims for the JWT.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject (user ID) of the JWT
    pub sub: String,
    /// Expiration time as Unix timestamp
    pub exp: usize,
}

const BEARER: &str = "Bearer ";
pub(crate) const MEDIA_AUTH_COOKIE: &str = "koko_media_auth";

pub(crate) fn set_media_auth_cookie(
    cookies: &CookieJar<'_>,
    token: &str,
) {
    cookies.add(
        Cookie::build((MEDIA_AUTH_COOKIE, token.to_string()))
            .path("/api/v1")
            .http_only(true)
            .same_site(SameSite::Strict)
            .secure(crate::config::current_settings().server.use_https)
            .max_age(rocket::time::Duration::hours(24))
            .build(),
    );
}

/// Create a JWT token.
pub fn create_token(
    user_id: &str,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
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
}

/// Decode a JWT token.
pub fn decode_token(
    token: &str,
    secret: &str,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_required_spec_claims(&["exp", "sub"]);

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    )
    .map(|data| data.claims)
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Claims {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let keys: Vec<_> = request.headers().get("Authorization").collect();
        if keys.len() > 1 {
            return Outcome::Error((rocket::http::Status::Unauthorized, ()));
        }

        // Native image/audio/video elements cannot attach Authorization headers.
        // Accept the HttpOnly login cookie only for read-only media asset routes.
        let path = request.uri().path().as_str();
        let media_asset = matches!(
            request.method(),
            rocket::http::Method::Get | rocket::http::Method::Head
        ) && ((path.starts_with("/api/v1/items/")
            && (path.ends_with("/artwork")
                || path.ends_with("/theme")
                || path.ends_with("/stream")
                || path.contains("/subtitles/")))
            || (path.starts_with("/api/v1/people/") && path.ends_with("/image"))
            || (path.starts_with("/api/v1/sessions/") && path.ends_with("/stream"))
            || (path.starts_with("/api/v1/podcasts/episodes/") && path.ends_with("/stream"))
            || ((path.starts_with("/api/v1/live-tv/channels/")
                || path.starts_with("/api/v1/live-tv/recordings/")
                || path.starts_with("/api/v1/live-tv/sessions/"))
                && path.ends_with("/stream"))
            || ((path.starts_with("/api/v1/live-tv/programs/")
                || path.starts_with("/api/v1/live-tv/channels/"))
                && path.ends_with("/artwork"))
            || (path.starts_with("/api/v1/live-tv/sources/") && path.ends_with("/image"))
            || path.starts_with("/api/v1/user-profile-images/"));
        let token = if let Some(header) = keys.first() {
            header.strip_prefix(BEARER)
        } else if media_asset {
            request
                .cookies()
                .get(MEDIA_AUTH_COOKIE)
                .map(|cookie| cookie.value())
        } else {
            None
        };
        let Some(token) = token else {
            return Outcome::Error((Status::Unauthorized, ()));
        };
        let secret = get_jwt_secret();

        match decode_token(token, secret) {
            Ok(claims) => Outcome::Success(claims),
            Err(_) => Outcome::Error((rocket::http::Status::Unauthorized, ())),
        }
    }
}

static JWT_SECRET: Lazy<String> = Lazy::new(|| {
    let random_bytes: [u8; 32] = rand::rng().random();
    general_purpose::STANDARD.encode(random_bytes)
});

pub(crate) fn get_jwt_secret() -> &'static str {
    &JWT_SECRET
}

/// Hash a password using BCrypt (handles salting internally)
pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    hash(password, DEFAULT_COST)
}

/// Verify a password against a BCrypt hash
pub fn verify_password(
    password: &str,
    hash: &str,
) -> bool {
    verify(password, hash).unwrap_or(false)
}
