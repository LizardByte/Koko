// lib imports
use diesel::{ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, SelectableHelper};
use rocket::get;
use rocket::http::Status;
use rocket::post;
use rocket::put;
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
    pub birthday: Option<String>,
    pub profile_image_url: Option<String>,
    pub preferred_metadata_languages: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateUserForm {
    pub username: String,
    pub admin: bool,
    pub birthday: Option<String>,
    pub profile_image_url: Option<String>,
    pub preferred_metadata_languages: Option<Vec<String>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UserSummary {
    pub id: i32,
    pub username: String,
    pub admin: bool,
    pub birthday: Option<String>,
    pub profile_image_url: Option<String>,
    pub preferred_metadata_languages: Vec<String>,
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
            birthday: user.birthday,
            profile_image_url: user.profile_image_url,
            preferred_metadata_languages: parse_preferred_metadata_languages(
                &user.preferred_metadata_languages_json,
            ),
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

    Ok(Json(
        users_list
            .into_iter()
            .map(|user| UserSummary {
                id: user.id,
                username: user.username,
                admin: user.admin,
                birthday: user.birthday,
                profile_image_url: user.profile_image_url,
                preferred_metadata_languages: parse_preferred_metadata_languages(
                    &user.preferred_metadata_languages_json,
                ),
            })
            .collect(),
    ))
}

#[openapi(tag = "Users")]
#[put(
    "/api/v1/users/<target_user_id>",
    format = "json",
    data = "<user_form>"
)]
pub async fn update_user(
    db: DbConn,
    _admin_guard: AdminGuard,
    target_user_id: i32,
    user_form: Json<UpdateUserForm>,
) -> Result<Json<UserSummary>, Status> {
    use crate::db::schema::users::dsl as users_dsl;

    let form = user_form.into_inner();
    let next_username = form.username.trim().to_string();
    if next_username.is_empty() {
        return Err(Status::BadRequest);
    }

    let next_birthday = form
        .birthday
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let next_profile_image_url = form
        .profile_image_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let next_preferred_languages = serialize_preferred_metadata_languages(
        form.preferred_metadata_languages
            .unwrap_or_else(default_preferred_metadata_languages),
    );

    let updated_user = db
        .run(move |conn| {
            let existing_user = users_dsl::users
                .filter(users_dsl::id.eq(target_user_id))
                .select(User::as_select())
                .first::<User>(conn)
                .optional()
                .map_err(|_| Status::InternalServerError)?
                .ok_or(Status::NotFound)?;

            if existing_user.admin && !form.admin {
                let admin_count = users_dsl::users
                    .filter(users_dsl::admin.eq(true))
                    .count()
                    .get_result::<i64>(conn)
                    .map_err(|_| Status::InternalServerError)?;
                if admin_count <= 1 {
                    return Err(Status::BadRequest);
                }
            }

            diesel::update(users_dsl::users.filter(users_dsl::id.eq(target_user_id)))
                .set((
                    users_dsl::username.eq(next_username),
                    users_dsl::admin.eq(form.admin),
                    users_dsl::birthday.eq(next_birthday),
                    users_dsl::profile_image_url.eq(next_profile_image_url),
                    users_dsl::preferred_metadata_languages_json.eq(next_preferred_languages),
                ))
                .execute(conn)
                .map_err(|_| Status::Conflict)?;

            users_dsl::users
                .filter(users_dsl::id.eq(target_user_id))
                .select(User::as_select())
                .first::<User>(conn)
                .map_err(|_| Status::InternalServerError)
        })
        .await?;

    Ok(Json(UserSummary {
        id: updated_user.id,
        username: updated_user.username,
        admin: updated_user.admin,
        birthday: updated_user.birthday,
        profile_image_url: updated_user.profile_image_url,
        preferred_metadata_languages: parse_preferred_metadata_languages(
            &updated_user.preferred_metadata_languages_json,
        ),
    }))
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
        birthday: form
            .birthday
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        profile_image_url: form
            .profile_image_url
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        preferred_metadata_languages_json: serialize_preferred_metadata_languages(
            form.preferred_metadata_languages
                .unwrap_or_else(default_preferred_metadata_languages),
        ),
    };

    // Insert new user
    db.run(move |conn| diesel::insert_into(users).values(&user).execute(conn))
        .await
        .map_err(|_| Status::InternalServerError)?;

    Ok("User created")
}

pub fn default_preferred_metadata_languages() -> Vec<String> {
    vec!["en-US".to_string()]
}

pub fn parse_preferred_metadata_languages(value: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(value)
        .unwrap_or_default()
        .into_iter()
        .map(|language| language.trim().to_string())
        .filter(|language| !language.is_empty())
        .fold(Vec::new(), |mut languages, language| {
            if !languages.contains(&language) {
                languages.push(language);
            }
            languages
        })
        .into_iter()
        .chain(default_preferred_metadata_languages())
        .fold(Vec::new(), |mut languages, language| {
            if !languages.contains(&language) {
                languages.push(language);
            }
            languages
        })
}

pub fn serialize_preferred_metadata_languages(languages: Vec<String>) -> String {
    serde_json::to_string(&parse_preferred_metadata_languages(
        &serde_json::to_string(&languages).unwrap_or_else(|_| "[]".into()),
    ))
    .unwrap_or_else(|_| "[\"en-US\"]".into())
}
