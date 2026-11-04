//! Database schema for the application.

diesel::table! {
    live_tv_sources (id) {
        id -> Integer,
        name -> Text,
        kind -> Text,
        config_json -> Text,
        enabled -> Bool,
        capacity -> Integer,
        next_refresh -> BigInt,
        last_error -> Nullable<Text>,
    }
}
diesel::table! {
    live_tv_channels (id) {
        id -> Integer,
        source_id -> Integer,
        source_key -> Text,
        name -> Text,
        number -> Text,
        group_name -> Text,
        logo -> Nullable<Text>,
        stream_url -> Text,
        guide_key -> Text,
        guide_override -> Nullable<Text>,
        enabled -> Bool,
        available -> Bool,
    }
}
diesel::table! {
    live_tv_programs (id) {
        id -> Integer,
        source_id -> Integer,
        guide_key -> Text,
        starts_at -> BigInt,
        ends_at -> BigInt,
        title -> Text,
        metadata_json -> Text,
    }
}
diesel::table! {
    live_tv_series_rules (id) {
        id -> Integer,
        user_id -> Integer,
        channel_id -> Integer,
        title -> Text,
        new_only -> Bool,
        padding_before -> Integer,
        padding_after -> Integer,
        enabled -> Bool,
        shared -> Bool,
    }
}
diesel::table! {
    live_tv_recordings (id) {
        id -> Integer,
        user_id -> Integer,
        channel_id -> Integer,
        rule_id -> Nullable<Integer>,
        title -> Text,
        starts_at -> BigInt,
        ends_at -> BigInt,
        padding_before -> Integer,
        padding_after -> Integer,
        state -> Text,
        file_name -> Nullable<Text>,
        last_error -> Nullable<Text>,
        metadata_json -> Text,
        shared -> Bool,
    }
}

diesel::table! {
    live_tv_user_permissions (user_id) {
        user_id -> Integer,
        can_record -> Bool,
    }
}
diesel::allow_tables_to_appear_in_same_query!(live_tv_user_permissions, users);
diesel::table! {
    live_tv_stations (source_id, guide_key) {
        source_id -> Integer,
        guide_key -> Text,
        name -> Text,
        number -> Text,
        aliases_json -> Text,
        logo -> Nullable<Text>,
    }
}
diesel::allow_tables_to_appear_in_same_query!(
    live_tv_sources,
    live_tv_channels,
    live_tv_programs,
    live_tv_stations,
    live_tv_series_rules,
    live_tv_recordings
);

// lib imports
use diesel::{
    allow_tables_to_appear_in_same_query,
    joinable,
    table,
};

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
        rating -> Nullable<SmallInt>,
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
    item_similarities (metadata_link_id, provider_id, media_type, external_id) {
        metadata_link_id -> Integer,
        provider_id -> Text,
        media_type -> Text,
        external_id -> Text,
        rank -> Integer,
    }
}

table! {
    item_recommendations (item_id, similar_item_id) {
        item_id -> Integer,
        similar_item_id -> Integer,
        provider_priority -> Integer,
        provider_rank -> Integer,
        local_score -> Integer,
        year_distance -> BigInt,
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
        metadata_match_attempted_at -> Nullable<BigInt>,
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
        use_embedded_music_tags -> Bool,
        exclude_from_home -> Bool,
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
joinable!(item_similarities -> item_metadata_links (metadata_link_id));
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
    item_similarities,
    item_recommendations,
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
    music_file_metadata,
    music_file_genres,
    music_item_metadata,
    music_item_genres,
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

table! {
    music_file_metadata (media_file_id) {
        media_file_id -> Integer,
        title -> Text,
        file_hash -> Text,
        artist -> Text,
        album_artist -> Text,
        album -> Text,
        track_number -> Nullable<Integer>,
        disc_number -> Nullable<Integer>,
        release_year -> Nullable<Integer>,
        musicbrainz_artist_id -> Nullable<Text>,
        musicbrainz_release_id -> Nullable<Text>,
        musicbrainz_recording_id -> Nullable<Text>,
    }
}

table! {
    music_file_genres (media_file_id, genre) {
        media_file_id -> Integer,
        genre -> Text,
    }
}

joinable!(music_file_metadata -> media_files (media_file_id));
joinable!(music_file_genres -> media_files (media_file_id));

table! {
    music_item_metadata (media_item_id) {
        media_item_id -> Integer,
        artist -> Text,
        album_artist -> Text,
        album -> Text,
        track_number -> Nullable<Integer>,
        disc_number -> Nullable<Integer>,
        release_year -> Nullable<Integer>,
        musicbrainz_artist_id -> Nullable<Text>,
        musicbrainz_release_id -> Nullable<Text>,
        musicbrainz_recording_id -> Nullable<Text>,
    }
}

table! {
    music_item_genres (media_item_id, source, genre) {
        media_item_id -> Integer,
        source -> Text,
        genre -> Text,
    }
}

joinable!(music_item_metadata -> media_items (media_item_id));
joinable!(music_item_genres -> media_items (media_item_id));

table! {
    music_playlists (id) {
        id -> Integer,
        user_id -> Integer,
        name -> Text,
        created_at -> BigInt,
        updated_at -> BigInt,
        temporary -> Bool,
        current_index -> Integer,
        position_ms -> BigInt,
        completed -> Bool,
        shuffle -> Bool,
        repeat_mode -> Text,
        artwork_item_id -> Nullable<Integer>,
    }
}
table! {
    music_playlist_entries (playlist_id, position) {
        playlist_id -> Integer,
        position -> Integer,
        media_item_id -> Integer,
    }
}
joinable!(music_playlists -> users (user_id));
joinable!(music_playlists -> media_items (artwork_item_id));
joinable!(music_playlist_entries -> music_playlists (playlist_id));
joinable!(music_playlist_entries -> media_items (media_item_id));
allow_tables_to_appear_in_same_query!(music_playlists, music_playlist_entries);
allow_tables_to_appear_in_same_query!(music_playlists, media_items);
allow_tables_to_appear_in_same_query!(music_playlists, users);
allow_tables_to_appear_in_same_query!(music_playlist_entries, media_items);
allow_tables_to_appear_in_same_query!(music_playlist_entries, users);

table! {
    music_progress_context (user_id, media_item_id) {
        user_id -> Integer,
        media_item_id -> Integer,
    }
}

table! {
    music_lyrics (media_item_id) {
        media_item_id -> Integer,
        track_title -> Text,
        artist_name -> Text,
        album_name -> Text,
        duration_ms -> Nullable<BigInt>,
        plain_lyrics -> Nullable<Text>,
        synced_lyrics -> Nullable<Text>,
        instrumental -> Bool,
        source -> Text,
        source_url -> Text,
        lookup_status -> Text,
        checked_at -> BigInt,
        next_check_at -> BigInt,
        last_error -> Nullable<Text>,
    }
}
joinable!(music_lyrics -> media_items (media_item_id));
allow_tables_to_appear_in_same_query!(music_lyrics, media_items);

table! {
    audiobook_metadata (media_item_id) {
        media_item_id -> Integer,
        metadata_json -> Text,
    }
}
joinable!(audiobook_metadata -> media_items (media_item_id));
allow_tables_to_appear_in_same_query!(audiobook_metadata, media_items);

table! {
    audiobook_provider_metadata (media_item_id, provider_id) {
        media_item_id -> Integer,
        provider_id -> Text,
        external_id -> Text,
        metadata_json -> Text,
    }
}
joinable!(audiobook_provider_metadata -> media_items (media_item_id));
allow_tables_to_appear_in_same_query!(audiobook_provider_metadata, media_items);

table! {
    audiobook_recording_selections (media_item_id) {
        media_item_id -> Integer,
        provider_id -> Text,
        external_id -> Text,
    }
}
joinable!(audiobook_recording_selections -> media_items (media_item_id));
allow_tables_to_appear_in_same_query!(audiobook_recording_selections, media_items);

table! {
    reading_progress (user_id, media_item_id) {
        user_id -> Integer,
        media_item_id -> Integer,
        payload_json -> Text,
        updated_at -> BigInt,
    }
}
joinable!(reading_progress -> media_items (media_item_id));
joinable!(reading_progress -> users (user_id));
allow_tables_to_appear_in_same_query!(reading_progress, media_items);
allow_tables_to_appear_in_same_query!(reading_progress, users);

table! {
    podcast_feeds (media_item_id) {
        media_item_id -> Integer,
        feed_url -> Text,
        checked_at -> BigInt,
        last_error -> Nullable<Text>,
    }
}
table! {
    podcast_feed_aliases (feed_url) {
        feed_url -> Text,
        podcast_id -> Integer,
    }
}
table! {
    podcast_subscriptions (user_id, podcast_id) {
        user_id -> Integer,
        podcast_id -> Integer,
        added_at -> BigInt,
    }
}
table! {
    podcast_items (media_item_id) {
        media_item_id -> Integer,
        podcast_id -> Integer,
        metadata_json -> Text,
        enclosure_url -> Nullable<Text>,
        mime_type -> Nullable<Text>,
    }
}
joinable!(podcast_feeds -> media_items (media_item_id));
joinable!(podcast_feed_aliases -> podcast_feeds (podcast_id));
joinable!(podcast_subscriptions -> podcast_feeds (podcast_id));
joinable!(podcast_subscriptions -> users (user_id));
joinable!(podcast_items -> podcast_feeds (podcast_id));
joinable!(podcast_items -> media_items (media_item_id));
allow_tables_to_appear_in_same_query!(
    podcast_feeds,
    podcast_feed_aliases,
    podcast_subscriptions,
    podcast_items
);
allow_tables_to_appear_in_same_query!(podcast_items, media_items);
allow_tables_to_appear_in_same_query!(podcast_feeds, media_items);
allow_tables_to_appear_in_same_query!(podcast_subscriptions, users);
