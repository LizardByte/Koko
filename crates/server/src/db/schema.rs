//! Database schema for the application.

// lib imports
use diesel::{allow_tables_to_appear_in_same_query, joinable, table};

table! {
    item_metadata_links (id) {
        id -> Integer,
        media_item_id -> Integer,
        provider_id -> Text,
        external_id -> Text,
        title -> Nullable<Text>,
        overview -> Nullable<Text>,
        tagline -> Nullable<Text>,
        artwork_url -> Nullable<Text>,
        backdrop_url -> Nullable<Text>,
        release_year -> Nullable<Integer>,
        media_type -> Nullable<Text>,
        relation_kind -> Text,
        match_state -> Text,
        provider_payload_json -> Nullable<Text>,
        cached_artwork_path -> Nullable<Text>,
        cached_backdrop_path -> Nullable<Text>,
        refresh_state -> Text,
        refresh_interval_seconds -> BigInt,
        last_refreshed_at -> Nullable<BigInt>,
        next_refresh_at -> Nullable<BigInt>,
        refresh_error -> Nullable<Text>,
        updated_at -> Nullable<BigInt>,
    }
}

table! {
    item_metadata_collections (id) {
        id -> Integer,
        metadata_link_id -> Integer,
        provider_id -> Text,
        external_id -> Text,
        name -> Text,
        overview -> Nullable<Text>,
        artwork_url -> Nullable<Text>,
        backdrop_url -> Nullable<Text>,
        provider_payload_json -> Nullable<Text>,
        updated_at -> Nullable<BigInt>,
    }
}

table! {
    item_metadata_people (id) {
        id -> Integer,
        metadata_link_id -> Integer,
        external_id -> Nullable<Text>,
        name -> Text,
        role -> Nullable<Text>,
        department -> Nullable<Text>,
        character_name -> Nullable<Text>,
        profile_url -> Nullable<Text>,
        image_url -> Nullable<Text>,
        sort_order -> Integer,
    }
}

table! {
    media_files (id) {
        id -> Integer,
        library_id -> Integer,
        source_root_path -> Text,
        relative_path -> Text,
        file_size -> BigInt,
        modified_at -> Nullable<BigInt>,
        media_kind -> Text,
        fingerprint_seed -> Text,
        display_title -> Nullable<Text>,
        container -> Nullable<Text>,
        duration_ms -> Nullable<BigInt>,
        bit_rate -> Nullable<BigInt>,
        width -> Nullable<Integer>,
        height -> Nullable<Integer>,
        video_codec -> Nullable<Text>,
        audio_codec -> Nullable<Text>,
        metadata_json -> Nullable<Text>,
        metadata_updated_at -> Nullable<BigInt>,
        metadata_match_attempted_at -> Nullable<BigInt>,
        media_item_id -> Nullable<Integer>,
    }
}

table! {
    media_items (id) {
        id -> Integer,
        library_id -> Integer,
        parent_id -> Nullable<Integer>,
        identity_key -> Text,
        item_type -> Text,
        display_title -> Text,
        relative_path -> Nullable<Text>,
        media_kind -> Nullable<Text>,
        season_number -> Nullable<Integer>,
        episode_number -> Nullable<Integer>,
        child_count -> Integer,
        playable -> Bool,
        file_size -> Nullable<BigInt>,
        duration_ms -> Nullable<BigInt>,
        modified_at -> Nullable<BigInt>,
        created_at -> Nullable<BigInt>,
        updated_at -> Nullable<BigInt>,
    }
}

table! {
    media_libraries (id) {
        id -> Integer,
        name -> Text,
        path -> Text,
        paths_json -> Text,
        kind -> Text,
        recursive -> Bool,
        metadata_providers_json -> Text,
    }
}

table! {
    playback_progress (id) {
        id -> Integer,
        user_id -> Nullable<Integer>,
        media_item_id -> Integer,
        position_ms -> BigInt,
        duration_ms -> Nullable<BigInt>,
        completed -> Bool,
        updated_at -> Nullable<BigInt>,
    }
}

table! {
    scan_state (id) {
        id -> Integer,
        library_id -> Integer,
        last_status -> Text,
        last_error -> Nullable<Text>,
        scan_revision -> BigInt,
        last_scanned_at -> Nullable<BigInt>,
        total_files -> BigInt,
        video_files -> BigInt,
        audio_files -> BigInt,
        image_files -> BigInt,
        book_files -> BigInt,
        other_files -> BigInt,
    }
}

joinable!(item_metadata_collections -> item_metadata_links (metadata_link_id));
joinable!(item_metadata_links -> media_items (media_item_id));
joinable!(item_metadata_people -> item_metadata_links (metadata_link_id));
joinable!(media_files -> media_libraries (library_id));
joinable!(media_files -> media_items (media_item_id));
joinable!(media_items -> media_libraries (library_id));
joinable!(playback_progress -> users (user_id));
joinable!(playback_progress -> media_items (media_item_id));
joinable!(scan_state -> media_libraries (library_id));

allow_tables_to_appear_in_same_query!(
    item_metadata_collections,
    item_metadata_links,
    item_metadata_people,
    media_files,
    media_items,
    media_libraries,
    playback_progress,
    scan_state,
    users
);

table! {
    users (id) {
        id -> Integer,
        username -> Text,
        password -> Text,
        pin -> Nullable<Text>,
        admin -> Bool,
        birthday -> Nullable<Text>,
        profile_image_url -> Nullable<Text>,
    }
}
