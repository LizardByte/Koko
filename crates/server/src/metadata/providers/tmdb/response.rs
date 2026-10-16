//! TMDB search response variants are selected by `media_type`, rather than by
//! trying unrelated title, date, or person-ID keys.
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "media_type")]
pub(super) enum MultiSearchResult {
    #[serde(rename = "movie")]
    Movie(MovieSearchResult),
    #[serde(rename = "tv")]
    Show(ShowSearchResult),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub(super) struct MovieSearchResult {
    pub id: i64,
    pub title: String,
    pub release_date: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct ShowSearchResult {
    pub id: i64,
    pub name: String,
    pub first_air_date: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
}

/// Identity fields shared by TMDB person details and cast/crew records.
#[derive(Deserialize)]
pub(super) struct PersonIdentity {
    pub id: Option<i64>,
    pub name: Option<String>,
}

/// Documented external-ID response fields. An arbitrary `*_id` key is not a
/// provider identity and must not be inferred to be one.
#[derive(Deserialize)]
pub(super) struct ExternalIds {
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i64>,
    pub wikidata_id: Option<String>,
    pub facebook_id: Option<String>,
    pub instagram_id: Option<String>,
    pub twitter_id: Option<String>,
    pub youtube_id: Option<String>,
    pub tiktok_id: Option<String>,
    pub freebase_id: Option<String>,
    pub freebase_mid: Option<String>,
    pub tvrage_id: Option<i64>,
}
