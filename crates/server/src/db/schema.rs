//! Database schema for the application.

// lib imports
use diesel::table;

table! {
    users (id) {
        id -> Integer,
        username -> Text,
        password -> Text,
        pin -> Nullable<Text>,
        admin -> Bool,
    }
}
