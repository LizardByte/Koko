//! Shared country choices from the versioned ISO 3166-1 dataset bundled with the server.

use crate::auth::UserGuard;
use once_cell::sync::Lazy;
use rocket::{
    get,
    serde::json::Json,
};
use rocket_okapi::{
    okapi::schemars::{
        self,
        JsonSchema,
    },
    openapi,
};
use serde::Serialize;
use unicode_normalization::{
    UnicodeNormalization,
    char::is_combining_mark,
};

/// A country or territory from ISO 3166-1, shared by all clients.
#[derive(Clone, Serialize, JsonSchema)]
pub struct Country {
    /// Lowercase ISO 3166-1 alpha-2 code, suitable for podcast directory requests.
    pub code: String,
    /// English ISO country name.
    pub name: &'static str,
}

static COUNTRIES: Lazy<Vec<Country>> = Lazy::new(|| {
    let mut countries: Vec<_> = rust_iso3166::ALL
        .iter()
        .map(|country| Country {
            code: country.alpha2.to_ascii_lowercase(),
            name: country.name,
        })
        .collect();
    // Treat accents as their base letters while preserving the original names in the response.
    countries.sort_by_cached_key(|country| {
        country
            .name
            .nfd()
            .filter(|c| !is_combining_mark(*c))
            .collect::<String>()
            .to_lowercase()
    });
    countries
});

/// List all ISO countries and territories in English alphabetical order.
///
/// This bundled list works offline and is independent of any provider's country availability.
#[openapi(tag = "Reference data")]
#[get("/api/v1/countries")]
pub fn get_countries(_user: UserGuard) -> Json<Vec<Country>> {
    Json(COUNTRIES.clone())
}
