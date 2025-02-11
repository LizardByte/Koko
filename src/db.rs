#![doc = "Database utilities for the application."]

// lib imports
use rocket_sync_db_pools::{database, diesel};

/// Database connection.
#[database("sqlite_db")]
pub struct DbConn(diesel::SqliteConnection);
