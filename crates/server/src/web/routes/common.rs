#![doc = "Routes for the web server."]

// lib imports
use rocket::get;
use rocket_okapi::openapi;

// local imports
use crate::globals;

#[openapi(tag = "Index")]
#[get("/")]
pub fn index() -> String {
    format!("Welcome to {}!", globals::GLOBAL_APP_NAME)
}
