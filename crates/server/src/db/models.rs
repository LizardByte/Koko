#![doc = "Database models for the application."]

// lib imports
use diesel::prelude::*;

// local imports
use crate::db::schema::users;

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = users)]
pub struct User {
    #[diesel(skip_insertion)]
    pub id: i32,
    pub username: String,
    pub password: String,
    pub pin: Option<String>,
    pub admin: bool,
}
