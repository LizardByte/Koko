// lib imports
use diesel::{QueryDsl, RunQueryDsl};
use rocket::http::Status;
use rocket::post;
use rocket::serde::{Deserialize, json::Json};
use rocket_okapi::JsonSchema;
use rocket_okapi::openapi;

// local imports
use crate::auth::AdminGuard;
use crate::db::DbConn;
use crate::db::models::User;

#[derive(Deserialize, JsonSchema)]
pub struct CreateUserForm {
    pub username: String,
    pub password: String,
    pub pin: Option<String>,
    pub admin: bool,
}

#[openapi(tag = "Users")]
#[post("/create_user", format = "json", data = "<user_form>")]
pub async fn create_user(
    db: DbConn,
    user_form: Json<CreateUserForm>,
    auth_guard: Option<AdminGuard>,
) -> Result<&'static str, Status> {
    use crate::db::schema::users::dsl::*;

    // Check if this is the first user (no authentication required)
    let existing_count = db
        .run(|conn| users.count().get_result::<i64>(conn))
        .await
        .unwrap_or(0);

    // If there are existing users, require admin privileges
    if existing_count > 0 && auth_guard.is_none() {
        return Err(Status::Unauthorized);
    }

    let form = user_form.into_inner();

    // Hash password using BCrypt
    let hashed_password = match crate::auth::hash_password(&form.password) {
        Ok(hash) => hash,
        Err(_) => return Err(Status::InternalServerError),
    };

    // Hash PIN if provided
    let hashed_pin = if let Some(pin_value) = form.pin {
        if pin_value.parse::<i32>().is_err() {
            return Err(Status::BadRequest);
        }
        if pin_value.len() < 4 || pin_value.len() > 6 {
            return Err(Status::BadRequest);
        }
        match crate::auth::hash_password(&pin_value) {
            Ok(hash) => Some(hash),
            Err(_) => return Err(Status::InternalServerError),
        }
    } else {
        None
    };

    let user = User {
        id: 0, // This will be auto-incremented by SQLite
        username: form.username,
        password: hashed_password,
        pin: hashed_pin,
        admin: form.admin,
    };

    // Insert new user
    db.run(move |conn| diesel::insert_into(users).values(&user).execute(conn))
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok("User created")
}
