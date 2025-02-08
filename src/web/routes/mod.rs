#![doc = "Common routes module for the web server."]

// modules
mod auth;
mod common;
mod dependencies;
mod user;

// lib imports
use rocket_okapi::openapi_get_routes; // this is a replacement for the rocket::routes macro

pub fn all_routes() -> Vec<rocket::Route> {
    openapi_get_routes![
        common::index,
        auth::login,
        auth::logout,
        auth::protected,
        dependencies::get_dependencies,
        user::create_user,
    ]
}
