//! Common routes module for the web server.

// modules
pub mod auth;
pub mod common;
pub mod countries;
pub mod dependencies;
pub mod ebooks;
pub mod live_tv;
pub mod media;
mod metadata_jobs;
pub mod music;
pub mod playlists;
pub mod podcasts;
pub mod settings;
pub mod user;

// lib imports
use rocket::routes;
use rocket_okapi::openapi_get_routes; // this is a replacement for the rocket::routes macro

pub fn api_routes() -> Vec<rocket::Route> {
    openapi_get_routes![
        auth::login,
        auth::logout,
        auth::jwt_test,
        auth::admin_test,
        auth::user_info,
        ebooks::document,
        ebooks::progress,
        live_tv::get_channels,
        live_tv::get_setup_channels,
        live_tv::get_guide,
        live_tv::guide_providers,
        live_tv::discover_tuners,
        live_tv::tuner_regions,
        live_tv::discover_sources,
        live_tv::scan_tuner,
        live_tv::get_sources,
        live_tv::add_source,
        live_tv::replace_source,
        live_tv::refresh_source,
        live_tv::set_source,
        live_tv::set_channel,
        live_tv::get_recordings,
        live_tv::get_dvr_settings,
        live_tv::set_dvr_settings,
        live_tv::set_recording_permission,
        live_tv::schedule_recording,
        live_tv::cancel_recording,
        live_tv::delete_recording,
        live_tv::get_rules,
        live_tv::delete_rule,
        dependencies::get_dependencies,
        countries::get_countries,
        media::get_server_capabilities,
        media::get_system_activities,
        media::get_metadata_providers,
        media::get_metadata_locales,
        media::get_home,
        podcasts::discover,
        podcasts::home,
        podcasts::get_user_settings,
        podcasts::set_user_settings,
        podcasts::season_episodes,
        podcasts::list,
        podcasts::episodes,
        podcasts::preferences,
        podcasts::add,
        podcasts::remove,
        podcasts::refresh,
        playlists::list,
        playlists::get,
        playlists::create,
        playlists::delete,
        playlists::update_queue,
        playlists::save_radio,
        music::lyrics,
        music::radio,
        media::get_libraries,
        media::scan_library,
        media::delete_library_missing_items,
        media::refresh_library_metadata,
        media::get_library_inventory,
        media::get_items,
        media::get_item,
        media::get_similar_items,
        media::get_item_files,
        media::get_item_credits,
        media::get_item_metadata,
        media::get_person,
        media::search_item_metadata,
        media::get_audiobook_recordings,
        media::link_item_metadata,
        media::unlink_item_metadata,
        media::refresh_item_metadata,
        media::get_item_playback,
        media::create_session,
        media::delete_session,
        media::get_session_status,
        media::search_items,
        media::update_item_progress,
        settings::get_settings,
        settings::get_logs,
        settings::clear_metadata_cache,
        settings::run_scheduled_task,
        settings::update_settings,
        settings::add_library,
        settings::remove_library,
        user::get_bootstrap,
        user::list_users,
        user::update_user,
        user::create_user,
    ]
}

pub fn spa_routes() -> Vec<rocket::Route> {
    routes![
        common::index,
        ebooks::content,
        common::favicon,
        common::provider_logo,
        common::spa_asset,
        user::get_user_profile_image,
        media::get_item_artwork,
        media::get_person_image,
        media::get_item_theme,
        media::get_item_subtitle,
        podcasts::stream,
        live_tv::stream_channel,
        live_tv::stream_recording,
        live_tv::upload_guide,
        media::stream_item,
        media::get_session_stream
    ]
}
