//! Common routes module for the web server.

// modules
pub mod auth;
pub mod books;
pub mod common;
pub mod dependencies;
pub mod media;
mod metadata_jobs;
pub mod music;
pub mod playlists;
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
        books::document,
        books::progress,
        dependencies::get_dependencies,
        media::get_server_capabilities,
        media::get_system_activities,
        media::get_metadata_providers,
        media::get_metadata_locales,
        media::get_home,
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
        books::content,
        common::favicon,
        common::provider_logo,
        common::spa_asset,
        user::get_user_profile_image,
        media::get_item_artwork,
        media::get_person_image,
        media::get_item_theme,
        media::get_item_subtitle,
        media::stream_item,
        media::get_session_stream
    ]
}
