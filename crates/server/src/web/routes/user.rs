// lib imports
use diesel::{QueryDsl, RunQueryDsl};
use rocket::http::Status;
use rocket::post;
use rocket::serde::{json::Json, Deserialize};
use rocket_okapi::openapi;
use rocket_okapi::JsonSchema;

// local imports
use crate::auth::AdminGuard;
use crate::db::models::User;
use crate::db::DbConn;

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
    admin_guard: Option<AdminGuard>,
) -> Result<&'static str, Status> {
    use crate::db::schema::users::dsl::*;
    let existing = db
        .run(|conn| users.count().get_result::<i64>(conn))
        .await
        .unwrap_or(0);

    // If there are users, require admin privileges
    if existing > 0 && admin_guard.is_none() {
        return Err(Status::Unauthorized);
    }

    let form = user_form.into_inner();
    let hashed_password = crate::auth::hash_password(&form.password);

    let hashed_pin = if let Some(pin_value) = form.pin {
        if pin_value.parse::<i32>().is_err() {
            return Err(Status::BadRequest);
        }
        if pin_value.len() < 4 || pin_value.len() > 6 {
            return Err(Status::BadRequest);
        }
        Some(crate::auth::hash_password(&pin_value))
    } else {
        None
    };

    let user = User {
        id: 0,
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
