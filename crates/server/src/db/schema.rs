#![doc = "Database schema for the application."]

// lib imports
use diesel::table;

table! {
    users (id) {
        id -> Integer,
        username -> Text,
        password -> Text,
        password_salt -> Text,
        pin -> Nullable<Text>,
        pin_salt -> Nullable<Text>,
        admin -> Bool,
    }
}
