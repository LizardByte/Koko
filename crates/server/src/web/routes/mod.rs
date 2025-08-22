//! Common routes module for the web server.

// modules
pub mod auth;
pub mod common;
pub mod dependencies;
pub mod user;

// lib imports
use rocket_okapi::openapi_get_routes; // this is a replacement for the rocket::routes macro

pub fn all_routes() -> Vec<rocket::Route> {
    openapi_get_routes![
        common::index,
        auth::login,
        auth::logout,
        auth::jwt_test,
        auth::admin_test,
        auth::user_info,
        dependencies::get_dependencies,
        user::create_user,
    ]
}
