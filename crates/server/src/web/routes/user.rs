// lib imports
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper};
use rocket::get;
use rocket::http::Status;
use rocket::post;
use rocket::serde::{Deserialize, Serialize, json::Json};
use rocket_okapi::JsonSchema;
use rocket_okapi::openapi;

// local imports
use crate::auth::{AdminGuard, UserGuard};
use crate::db::DbConn;
use crate::db::models::User;

#[derive(Deserialize, JsonSchema)]
pub struct CreateUserForm {
    pub username: String,
    pub password: String,
    pub pin: Option<String>,
    pub admin: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UserSummary {
    pub id: i32,
    pub username: String,
    pub admin: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BootstrapResponse {
    pub has_users: bool,
    pub current_user: Option<UserSummary>,
}

#[openapi(tag = "Users")]
#[get("/api/v1/bootstrap")]
pub async fn get_bootstrap(
    db: DbConn,
    user_guard: Option<UserGuard>,
) -> Result<Json<BootstrapResponse>, Status> {
    use crate::db::schema::users::dsl::*;

    let has_users = db
        .run(|conn| users.count().get_result::<i64>(conn))
        .await
        .map_err(|_| Status::InternalServerError)?
        > 0;

    let current_user = if let Some(user_guard) = user_guard {
        let user_id = user_guard
            .claims()
            .sub
            .parse::<i32>()
            .map_err(|_| Status::Unauthorized)?;
        db.run(move |conn| {
            users
                .filter(id.eq(user_id))
                .select(User::as_select())
                .first::<User>(conn)
                .optional()
        })
        .await
        .map_err(|_| Status::InternalServerError)?
        .map(|user| UserSummary {
            id: user.id,
            username: user.username,
            admin: user.admin,
        })
    } else {
        None
    };

    Ok(Json(BootstrapResponse {
        has_users,
        current_user,
    }))
}

#[openapi(tag = "Users")]
#[get("/api/v1/users")]
pub async fn list_users(
    db: DbConn,
    _admin_guard: AdminGuard,
) -> Result<Json<Vec<UserSummary>>, Status> {
    use crate::db::schema::users::dsl::*;

    let users_list = db
        .run(|conn| {
            users
                .order(username.asc())
                .select(User::as_select())
                .load::<User>(conn)
        })
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok(Json(users_list
        .into_iter()
        .map(|user| UserSummary {
            id: user.id,
            username: user.username,
            admin: user.admin,
        })
        .collect()))
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
        admin: existing_count == 0 || form.admin,
    };

    // Insert new user
    db.run(move |conn| diesel::insert_into(users).values(&user).execute(conn))
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok("User created")
}
