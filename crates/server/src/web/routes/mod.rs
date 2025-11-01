//! Common routes module for the web server.

// modules
pub mod auth;
pub mod common;
pub mod dependencies;
#[cfg(target_os = "linux")]
pub mod streaming;
pub mod user;

// lib imports
use rocket_okapi::openapi_get_routes; // this is a replacement for the rocket::routes macro

#[cfg(target_os = "linux")]
use rocket::routes;

pub fn all_routes() -> Vec<rocket::Route> {
    let mut routes = openapi_get_routes![
        common::index,
        auth::login,
        auth::logout,
        auth::jwt_test,
        auth::admin_test,
        auth::user_info,
        dependencies::get_dependencies,
        user::create_user,
    ];
    
    // Add WebSocket routes (not in OpenAPI) - Linux only
    #[cfg(target_os = "linux")]
    routes.extend(routes![streaming::stream]);
    
    routes
}
