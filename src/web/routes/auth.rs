#![doc = "Routes for the web server."]

// lib imports
use rocket::get;
use rocket_okapi::openapi;

use crate::auth::Claims;

#[openapi(tag = "Auth")]
#[get("/login")]
pub fn login() -> &'static str {
    "Login Page"
}

#[openapi(tag = "Auth")]
#[get("/logout")]
pub fn logout() -> &'static str {
    "Logout Page"
}

#[openapi(tag = "Auth")]
#[get("/protected")]
pub fn protected(_claims: Claims) -> &'static str {
    "Protected Page"
}
