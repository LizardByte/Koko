//! Database schema for the application.

// lib imports
use diesel::{allow_tables_to_appear_in_same_query, joinable, table};

table! {
    app_settings (key) {
        key -> Text,
        value -> Text,
        updated_at -> Nullable<BigInt>,
    }
}

table! {
    external_media (id) {
        id -> Integer,
        source -> Text,
        external_id -> Nullable<Text>,
        url -> Text,
        media_kind -> Text,
        title -> Nullable<Text>,
        duration_seconds -> Nullable<Integer>,
        thumbnail_url -> Nullable<Text>,
        updated_at -> Nullable<BigInt>,
    }
}

table! {
    item_metadata_external_ids (id) {
        id -> Integer,
        metadata_link_id -> Integer,
        source -> Text,
        external_id -> Text,
        updated_at -> Nullable<BigInt>,
    }
}

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
        logo_url -> Nullable<Text>,
        cached_logo_path -> Nullable<Text>,
        genres_json -> Nullable<Text>,
        rating -> Nullable<Float>,
        content_rating -> Nullable<Text>,
        locale_key -> Text,
        provider_locale_key -> Nullable<Text>,
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
    metadata_collections (id) {
        id -> Integer,
        provider_id -> Text,
        external_id -> Text,
        source_provider_id -> Text,
        source_external_id -> Text,
        relation_kind -> Text,
        locale_key -> Text,
        provider_locale_key -> Nullable<Text>,
        name -> Nullable<Text>,
        overview -> Nullable<Text>,
        artwork_url -> Nullable<Text>,
        backdrop_url -> Nullable<Text>,
        updated_at -> Nullable<BigInt>,
    }
}

table! {
    metadata_collection_items (id) {
        id -> Integer,
        collection_id -> Integer,
        media_item_id -> Integer,
        metadata_link_id -> Integer,
        updated_at -> Nullable<BigInt>,
    }
}

table! {
    metadata_extras (id) {
        id -> Integer,
        metadata_link_id -> Nullable<Integer>,
        collection_id -> Nullable<Integer>,
        external_media_id -> Integer,
        extra_type -> Text,
        title -> Nullable<Text>,
        sort_order -> Integer,
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
    metadata_people (id) {
        id -> Integer,
        provider_id -> Text,
        external_id -> Nullable<Text>,
        locale_key -> Text,
        name -> Text,
        known_for_json -> Nullable<Text>,
        biography -> Nullable<Text>,
        gender -> Nullable<Text>,
        birthday -> Nullable<Text>,
        deathday -> Nullable<Text>,
        birth_place -> Nullable<Text>,
        profile_url -> Nullable<Text>,
        image_url -> Nullable<Text>,
        cached_image_path -> Nullable<Text>,
        updated_at -> Nullable<BigInt>,
    }
}

table! {
    metadata_person_external_ids (id) {
        id -> Integer,
        person_id -> Integer,
        source -> Text,
        external_id -> Text,
        updated_at -> Nullable<BigInt>,
    }
}

table! {
    metadata_person_credits (id) {
        id -> Integer,
        metadata_link_id -> Integer,
        person_id -> Integer,
        role -> Nullable<Text>,
        department -> Nullable<Text>,
        character_name -> Nullable<Text>,
        sort_order -> Integer,
    }
}

table! {
    media_files (id) {
        id -> Integer,
        path -> Text,
        file_size -> BigInt,
        modified_at -> Nullable<BigInt>,
        media_kind -> Text,
        file_hash -> Text,
        container -> Nullable<Text>,
        duration_ms -> Nullable<BigInt>,
        bit_rate -> Nullable<BigInt>,
        width -> Nullable<Integer>,
        height -> Nullable<Integer>,
        video_codec -> Nullable<Text>,
        audio_codec -> Nullable<Text>,
        metadata_json -> Nullable<Text>,
        metadata_updated_at -> Nullable<BigInt>,
    }
}

table! {
    media_file_libraries (id) {
        id -> Integer,
        media_file_id -> Integer,
        library_id -> Integer,
        source_root_path -> Text,
        relative_path -> Text,
        display_title -> Nullable<Text>,
        metadata_match_attempted_at -> Nullable<BigInt>,
        media_item_id -> Nullable<Integer>,
        missing_since -> Nullable<BigInt>,
        deleted_at -> Nullable<BigInt>,
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
        missing_since -> Nullable<BigInt>,
        deleted_at -> Nullable<BigInt>,
    }
}

table! {
    media_libraries (id) {
        id -> Integer,
        name -> Text,
        path -> Text,
        paths_json -> Text,
        kind -> Text,
        scanner -> Text,
        recursive -> Bool,
        metadata_providers_json -> Text,
        metadata_language_mode -> Text,
        metadata_languages_json -> Text,
        allowed_user_ids_json -> Text,
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
        watch_count -> Integer,
        last_watched_at -> Nullable<BigInt>,
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

joinable!(metadata_extras -> external_media (external_media_id));
joinable!(metadata_collection_items -> item_metadata_links (metadata_link_id));
joinable!(metadata_collection_items -> media_items (media_item_id));
joinable!(metadata_collection_items -> metadata_collections (collection_id));
joinable!(item_metadata_external_ids -> item_metadata_links (metadata_link_id));
joinable!(item_metadata_links -> media_items (media_item_id));
joinable!(item_metadata_people -> item_metadata_links (metadata_link_id));
joinable!(metadata_person_credits -> item_metadata_links (metadata_link_id));
joinable!(metadata_person_credits -> metadata_people (person_id));
joinable!(metadata_person_external_ids -> metadata_people (person_id));
joinable!(media_file_libraries -> media_files (media_file_id));
joinable!(media_file_libraries -> media_libraries (library_id));
joinable!(media_file_libraries -> media_items (media_item_id));
joinable!(media_items -> media_libraries (library_id));
joinable!(playback_progress -> users (user_id));
joinable!(playback_progress -> media_items (media_item_id));
joinable!(scan_state -> media_libraries (library_id));

allow_tables_to_appear_in_same_query!(
    app_settings,
    external_media,
    item_metadata_external_ids,
    item_metadata_links,
    item_metadata_people,
    metadata_collection_items,
    metadata_collections,
    metadata_extras,
    metadata_people,
    metadata_person_credits,
    metadata_person_external_ids,
    media_file_libraries,
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
        profile_image_path -> Nullable<Text>,
        preferred_metadata_languages_json -> Text,
    }
}
