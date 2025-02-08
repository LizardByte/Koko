// lib imports
use diesel::{Insertable, QueryDsl, RunQueryDsl};
use rocket::http::Status;
use rocket::post;
use rocket::serde::json::Json;
use rocket_okapi::openapi;
use rocket_okapi::JsonSchema;
use serde::Deserialize;

// local imports
use crate::auth::Claims;
use crate::db::schema::users;
use crate::db::DbConn;

#[derive(Deserialize, JsonSchema)]
pub struct UserForm {
    pub username: String,
    pub password: String,
    pub pin: Option<String>,
    pub admin: bool,
}

#[derive(Insertable)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub username: String,
    pub password: String,
    pub password_salt: String,
    pub pin: Option<String>,
    pub pin_salt: Option<String>,
    pub admin: bool,
}

#[openapi(tag = "Users")]
#[post("/create_user", format = "json", data = "<user_form>")]
pub async fn create_user(
    db: DbConn,
    user_form: Json<UserForm>,
    claims: Option<Claims>,
) -> Result<&'static str, Status> {
    use crate::db::schema::users::dsl::*;
    let existing = db
        .run(|conn| users.count().get_result::<i64>(conn))
        .await
        .unwrap_or(0);

    // If there are any users, enforce claims
    if existing > 0 && claims.is_none() {
        return Err(Status::Unauthorized);
    }

    let form = user_form.into_inner();
    let salt_pw = crate::auth::generate_salt();
    let hashed_password = crate::auth::hash_with_salt(salt_pw.clone(), &form.password);

    // ensure pin is at least 4 digits and less than 6 digits, only if the pin is provided
    let (hashed_pin, salt_pin) = if let Some(pin_value) = form.pin {
        // ensure pin string could be cast to a number
        if pin_value.parse::<i32>().is_err() {
            return Err(Status::BadRequest);
        }

        // ensure pin string length is between 4 and 6
        if pin_value.len() < 4 || pin_value.len() > 6 {
            return Err(Status::BadRequest);
        }

        let salt = crate::auth::generate_salt();
        let hash = crate::auth::hash_with_salt(salt.clone(), pin_value.as_str());
        (Some(hash), Some(salt))
    } else {
        (None, None)
    };

    let new_user = NewUser {
        username: form.username,
        password: hashed_password,
        password_salt: salt_pw,
        pin: hashed_pin,
        pin_salt: salt_pin,
        admin: form.admin,
    };

    // Insert new user
    db.run(move |conn| diesel::insert_into(users).values(&new_user).execute(conn))
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok("User created")
}
