//! Database models for the application.

// lib imports
use diesel::prelude::*;

// local imports
use crate::db::schema::{
    item_metadata_external_ids, item_metadata_links, item_metadata_people, media_files,
    media_items, media_libraries, metadata_collection_items, metadata_collections, metadata_people,
    metadata_person_credits, playback_progress, scan_state, users,
};

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = users)]
pub struct User {
    #[diesel(skip_insertion)]
    pub id: i32,
    pub username: String,
    pub password: String,
    pub pin: Option<String>,
    pub admin: bool,
    pub birthday: Option<String>,
    pub profile_image_url: Option<String>,
    pub preferred_metadata_languages_json: String,
}

#[derive(Queryable, Selectable, Identifiable, Debug, Clone)]
#[diesel(table_name = media_libraries)]
pub struct MediaLibrary {
    pub id: i32,
    pub name: String,
    pub path: String,
    pub paths_json: String,
    pub kind: String,
    pub recursive: bool,
    pub metadata_providers_json: String,
    pub metadata_language_mode: String,
    pub metadata_languages_json: String,
    pub allowed_user_ids_json: String,
}

#[derive(Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = media_libraries)]
pub struct NewMediaLibrary {
    pub name: String,
    pub path: String,
    pub paths_json: String,
    pub kind: String,
    pub recursive: bool,
    pub metadata_providers_json: String,
    pub metadata_language_mode: String,
    pub metadata_languages_json: String,
    pub allowed_user_ids_json: String,
}

#[derive(Queryable, Selectable, Identifiable, Associations, Debug)]
#[diesel(belongs_to(MediaLibrary, foreign_key = library_id))]
#[diesel(table_name = scan_state)]
pub struct ScanState {
    pub id: i32,
    pub library_id: i32,
    pub last_status: String,
    pub last_error: Option<String>,
    pub scan_revision: i64,
    pub last_scanned_at: Option<i64>,
    pub total_files: i64,
    pub video_files: i64,
    pub audio_files: i64,
    pub image_files: i64,
    pub book_files: i64,
    pub other_files: i64,
}

#[derive(Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = scan_state)]
#[diesel(treat_none_as_null = true)]
pub struct NewScanState {
    pub library_id: i32,
    pub last_status: String,
    pub last_error: Option<String>,
    pub scan_revision: i64,
    pub last_scanned_at: Option<i64>,
    pub total_files: i64,
    pub video_files: i64,
    pub audio_files: i64,
    pub image_files: i64,
    pub book_files: i64,
    pub other_files: i64,
}

#[derive(Queryable, Selectable, Identifiable, Associations, Debug)]
#[diesel(belongs_to(MediaLibrary, foreign_key = library_id))]
#[diesel(table_name = media_files)]
pub struct MediaFile {
    pub id: i32,
    pub library_id: i32,
    pub source_root_path: String,
    pub relative_path: String,
    pub file_size: i64,
    pub modified_at: Option<i64>,
    pub media_kind: String,
    pub fingerprint_seed: String,
    pub display_title: Option<String>,
    pub container: Option<String>,
    pub duration_ms: Option<i64>,
    pub bit_rate: Option<i64>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub metadata_json: Option<String>,
    pub metadata_updated_at: Option<i64>,
    pub metadata_match_attempted_at: Option<i64>,
    pub media_item_id: Option<i32>,
}

#[derive(Queryable, Selectable, Identifiable, Associations, Debug, Clone)]
#[diesel(belongs_to(MediaLibrary, foreign_key = library_id))]
#[diesel(belongs_to(MediaItem, foreign_key = parent_id))]
#[diesel(table_name = media_items)]
pub struct MediaItem {
    pub id: i32,
    pub library_id: i32,
    pub parent_id: Option<i32>,
    pub identity_key: String,
    pub item_type: String,
    pub display_title: String,
    pub relative_path: Option<String>,
    pub media_kind: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub child_count: i32,
    pub playable: bool,
    pub file_size: Option<i64>,
    pub duration_ms: Option<i64>,
    pub modified_at: Option<i64>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

#[derive(Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = media_items)]
#[diesel(treat_none_as_null = true)]
pub struct NewMediaItem {
    pub library_id: i32,
    pub parent_id: Option<i32>,
    pub identity_key: String,
    pub item_type: String,
    pub display_title: String,
    pub relative_path: Option<String>,
    pub media_kind: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub child_count: i32,
    pub playable: bool,
    pub file_size: Option<i64>,
    pub duration_ms: Option<i64>,
    pub modified_at: Option<i64>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

#[derive(Queryable, Selectable, Identifiable, Associations, Debug, Clone)]
#[diesel(belongs_to(MediaItem, foreign_key = media_item_id))]
#[diesel(table_name = item_metadata_links)]
pub struct ItemMetadataLink {
    pub id: i32,
    pub media_item_id: i32,
    pub provider_id: String,
    pub external_id: String,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub tagline: Option<String>,
    pub artwork_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub release_year: Option<i32>,
    pub media_type: Option<String>,
    pub relation_kind: String,
    pub match_state: String,
    pub logo_url: Option<String>,
    pub cached_logo_path: Option<String>,
    pub genres_json: Option<String>,
    pub rating: Option<f32>,
    pub content_rating: Option<String>,
    pub trailer_title: Option<String>,
    pub trailer_url: Option<String>,
    pub theme_song_url: Option<String>,
    pub locale_key: String,
    pub provider_locale_key: Option<String>,
    pub cached_artwork_path: Option<String>,
    pub cached_backdrop_path: Option<String>,
    pub refresh_state: String,
    pub refresh_interval_seconds: i64,
    pub last_refreshed_at: Option<i64>,
    pub next_refresh_at: Option<i64>,
    pub refresh_error: Option<String>,
    pub updated_at: Option<i64>,
}

#[derive(Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = item_metadata_links)]
#[diesel(treat_none_as_null = true)]
pub struct NewItemMetadataLink {
    pub media_item_id: i32,
    pub provider_id: String,
    pub external_id: String,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub tagline: Option<String>,
    pub artwork_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub release_year: Option<i32>,
    pub media_type: Option<String>,
    pub relation_kind: String,
    pub match_state: String,
    pub logo_url: Option<String>,
    pub cached_logo_path: Option<String>,
    pub genres_json: Option<String>,
    pub rating: Option<f32>,
    pub content_rating: Option<String>,
    pub trailer_title: Option<String>,
    pub trailer_url: Option<String>,
    pub theme_song_url: Option<String>,
    pub locale_key: String,
    pub provider_locale_key: Option<String>,
    pub cached_artwork_path: Option<String>,
    pub cached_backdrop_path: Option<String>,
    pub refresh_state: String,
    pub refresh_interval_seconds: i64,
    pub last_refreshed_at: Option<i64>,
    pub next_refresh_at: Option<i64>,
    pub refresh_error: Option<String>,
    pub updated_at: Option<i64>,
}

#[derive(Queryable, Selectable, Identifiable, Associations, Debug, Clone)]
#[diesel(belongs_to(ItemMetadataLink, foreign_key = metadata_link_id))]
#[diesel(table_name = item_metadata_external_ids)]
pub struct ItemMetadataExternalId {
    pub id: i32,
    pub metadata_link_id: i32,
    pub source: String,
    pub external_id: String,
    pub updated_at: Option<i64>,
}

#[derive(Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = item_metadata_external_ids)]
#[diesel(treat_none_as_null = true)]
pub struct NewItemMetadataExternalId {
    pub metadata_link_id: i32,
    pub source: String,
    pub external_id: String,
    pub updated_at: Option<i64>,
}

#[derive(Queryable, Selectable, Identifiable, Associations, Debug, Clone)]
#[diesel(belongs_to(ItemMetadataLink, foreign_key = metadata_link_id))]
#[diesel(table_name = item_metadata_people)]
pub struct ItemMetadataPerson {
    pub id: i32,
    pub metadata_link_id: i32,
    pub external_id: Option<String>,
    pub name: String,
    pub role: Option<String>,
    pub department: Option<String>,
    pub character_name: Option<String>,
    pub profile_url: Option<String>,
    pub image_url: Option<String>,
    pub sort_order: i32,
}

#[derive(Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = item_metadata_people)]
#[diesel(treat_none_as_null = true)]
pub struct NewItemMetadataPerson {
    pub metadata_link_id: i32,
    pub external_id: Option<String>,
    pub name: String,
    pub role: Option<String>,
    pub department: Option<String>,
    pub character_name: Option<String>,
    pub profile_url: Option<String>,
    pub image_url: Option<String>,
    pub sort_order: i32,
}

#[derive(Queryable, Selectable, Identifiable, Debug, Clone)]
#[diesel(table_name = metadata_people)]
pub struct MetadataPerson {
    pub id: i32,
    pub provider_id: String,
    pub external_id: Option<String>,
    pub identity_key: String,
    pub locale_key: String,
    pub name: String,
    pub known_for_json: Option<String>,
    pub biography: Option<String>,
    pub gender: Option<String>,
    pub birthday: Option<String>,
    pub deathday: Option<String>,
    pub birth_place: Option<String>,
    pub profile_url: Option<String>,
    pub image_url: Option<String>,
    pub cached_image_path: Option<String>,
    pub updated_at: Option<i64>,
}

#[derive(Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = metadata_people)]
#[diesel(treat_none_as_null = true)]
pub struct NewMetadataPerson {
    pub provider_id: String,
    pub external_id: Option<String>,
    pub identity_key: String,
    pub locale_key: String,
    pub name: String,
    pub known_for_json: Option<String>,
    pub biography: Option<String>,
    pub gender: Option<String>,
    pub birthday: Option<String>,
    pub deathday: Option<String>,
    pub birth_place: Option<String>,
    pub profile_url: Option<String>,
    pub image_url: Option<String>,
    pub cached_image_path: Option<String>,
    pub updated_at: Option<i64>,
}

#[derive(Queryable, Selectable, Identifiable, Associations, Debug, Clone)]
#[diesel(belongs_to(ItemMetadataLink, foreign_key = metadata_link_id))]
#[diesel(belongs_to(MetadataPerson, foreign_key = person_id))]
#[diesel(table_name = metadata_person_credits)]
pub struct MetadataPersonCredit {
    pub id: i32,
    pub metadata_link_id: i32,
    pub person_id: i32,
    pub role: Option<String>,
    pub department: Option<String>,
    pub character_name: Option<String>,
    pub sort_order: i32,
}

#[derive(Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = metadata_person_credits)]
#[diesel(treat_none_as_null = true)]
pub struct NewMetadataPersonCredit {
    pub metadata_link_id: i32,
    pub person_id: i32,
    pub role: Option<String>,
    pub department: Option<String>,
    pub character_name: Option<String>,
    pub sort_order: i32,
}

#[derive(Queryable, Selectable, Identifiable, Debug, Clone)]
#[diesel(table_name = metadata_collections)]
pub struct MetadataCollection {
    pub id: i32,
    pub provider_id: String,
    pub external_id: String,
    pub source_provider_id: String,
    pub source_external_id: String,
    pub relation_kind: String,
    pub locale_key: String,
    pub provider_locale_key: Option<String>,
    pub name: String,
    pub overview: Option<String>,
    pub artwork_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub theme_song_url: Option<String>,
    pub updated_at: Option<i64>,
}

#[derive(Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = metadata_collections)]
#[diesel(treat_none_as_null = true)]
pub struct NewMetadataCollection {
    pub provider_id: String,
    pub external_id: String,
    pub source_provider_id: String,
    pub source_external_id: String,
    pub relation_kind: String,
    pub locale_key: String,
    pub provider_locale_key: Option<String>,
    pub name: String,
    pub overview: Option<String>,
    pub artwork_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub theme_song_url: Option<String>,
    pub updated_at: Option<i64>,
}

#[derive(Queryable, Selectable, Identifiable, Associations, Debug, Clone)]
#[diesel(belongs_to(MetadataCollection, foreign_key = collection_id))]
#[diesel(belongs_to(MediaItem, foreign_key = media_item_id))]
#[diesel(belongs_to(ItemMetadataLink, foreign_key = metadata_link_id))]
#[diesel(table_name = metadata_collection_items)]
pub struct MetadataCollectionItem {
    pub id: i32,
    pub collection_id: i32,
    pub media_item_id: i32,
    pub metadata_link_id: i32,
    pub updated_at: Option<i64>,
}

#[derive(Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = metadata_collection_items)]
#[diesel(treat_none_as_null = true)]
pub struct NewMetadataCollectionItem {
    pub collection_id: i32,
    pub media_item_id: i32,
    pub metadata_link_id: i32,
    pub updated_at: Option<i64>,
}

#[derive(Queryable, Selectable, Identifiable, Associations, Debug)]
#[diesel(belongs_to(MediaItem, foreign_key = media_item_id))]
#[diesel(table_name = playback_progress)]
pub struct PlaybackProgress {
    pub id: i32,
    pub user_id: Option<i32>,
    pub media_item_id: i32,
    pub position_ms: i64,
    pub duration_ms: Option<i64>,
    pub completed: bool,
    pub updated_at: Option<i64>,
}

#[derive(Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = playback_progress)]
#[diesel(treat_none_as_null = true)]
pub struct NewPlaybackProgress {
    pub user_id: i32,
    pub media_item_id: i32,
    pub position_ms: i64,
    pub duration_ms: Option<i64>,
    pub completed: bool,
    pub updated_at: Option<i64>,
}

#[derive(Insertable, AsChangeset, Debug, Clone)]
#[diesel(table_name = media_files)]
#[diesel(treat_none_as_null = true)]
pub struct NewMediaFile {
    pub library_id: i32,
    pub source_root_path: String,
    pub relative_path: String,
    pub file_size: i64,
    pub modified_at: Option<i64>,
    pub media_kind: String,
    pub fingerprint_seed: String,
    pub display_title: Option<String>,
    pub container: Option<String>,
    pub duration_ms: Option<i64>,
    pub bit_rate: Option<i64>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub metadata_json: Option<String>,
    pub metadata_updated_at: Option<i64>,
    pub metadata_match_attempted_at: Option<i64>,
    pub media_item_id: Option<i32>,
}
