// lib imports
use std::path::{
    Path,
    PathBuf,
};
use std::sync::atomic::Ordering;
use std::time::{
    SystemTime,
    UNIX_EPOCH,
};

use base64::{
    Engine as _,
    engine::general_purpose,
};
use diesel::{
    ExpressionMethods,
    OptionalExtension,
    QueryDsl,
    RunQueryDsl,
    SelectableHelper,
};
use rocket::fs::NamedFile;
use rocket::get;
use rocket::http::Status;
use rocket::post;
use rocket::put;
use rocket::serde::{
    Deserialize,
    Serialize,
    json::Json,
};
use rocket::tokio::fs;
use rocket_okapi::JsonSchema;
use rocket_okapi::openapi;
use sha2::{
    Digest,
    Sha256,
};

// local imports
use crate::auth::{
    AdminGuard,
    UserGuard,
};
use crate::config::current_settings;
use crate::db::DbConn;
use crate::db::models::User;
use crate::globals::{
    CURRENT_ENV,
    Environment,
};

const PROFILE_IMAGE_MAX_BYTES: usize = 2 * 1024 * 1024;
const PROFILE_IMAGE_ROUTE_PREFIX: &str = "/api/v1/user-profile-images/";

#[derive(Deserialize, JsonSchema)]
pub struct CreateUserForm {
    pub username: String,
    pub password: String,
    pub pin: Option<String>,
    pub admin: bool,
    pub birthday: Option<String>,
    pub profile_image_upload: Option<ProfileImageUploadForm>,
    pub preferred_metadata_languages: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct UpdateUserForm {
    pub username: String,
    pub admin: bool,
    pub birthday: Option<String>,
    pub profile_image_upload: Option<ProfileImageUploadForm>,
    pub remove_profile_image: Option<bool>,
    pub preferred_metadata_languages: Option<Vec<String>>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ProfileImageUploadForm {
    pub mime_type: String,
    pub data_base64: String,
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
        .map(user_summary)
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

    Ok(Json(users_list.into_iter().map(user_summary).collect()))
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
    let next_admin = form.admin;

    let next_birthday = form
        .birthday
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let profile_image_upload = form.profile_image_upload;
    let remove_profile_image = form.remove_profile_image.unwrap_or(false);
    let next_preferred_languages = serialize_preferred_metadata_languages(
        form.preferred_metadata_languages
            .unwrap_or_else(default_preferred_metadata_languages),
    );
    let conflict_username = next_username.clone();

    let existing_profile_image_path = db
        .run(move |conn| {
            let existing_user = users_dsl::users
                .filter(users_dsl::id.eq(target_user_id))
                .select(User::as_select())
                .first::<User>(conn)
                .optional()
                .map_err(|_| Status::InternalServerError)?
                .ok_or(Status::NotFound)?;

            if existing_user.admin && !next_admin {
                let admin_count = users_dsl::users
                    .filter(users_dsl::admin.eq(true))
                    .count()
                    .get_result::<i64>(conn)
                    .map_err(|_| Status::InternalServerError)?;
                if admin_count <= 1 {
                    return Err(Status::BadRequest);
                }
            }

            let conflicting_username = users_dsl::users
                .filter(users_dsl::id.ne(target_user_id))
                .filter(users_dsl::username.eq(&conflict_username))
                .count()
                .get_result::<i64>(conn)
                .map_err(|_| Status::InternalServerError)?
                > 0;
            if conflicting_username {
                return Err(Status::Conflict);
            }

            Ok(existing_user.profile_image_path)
        })
        .await?;

    let (next_profile_image_path, pending_uploaded_image_path) =
        if let Some(upload) = profile_image_upload {
            let uploaded_path = store_profile_image(upload).await?;
            (Some(uploaded_path.clone()), Some(uploaded_path))
        } else if remove_profile_image {
            (None, None)
        } else {
            (existing_profile_image_path.clone(), None)
        };

    let update_result = db
        .run(move |conn| {
            diesel::update(users_dsl::users.filter(users_dsl::id.eq(target_user_id)))
                .set((
                    users_dsl::username.eq(next_username),
                    users_dsl::admin.eq(next_admin),
                    users_dsl::birthday.eq(next_birthday),
                    users_dsl::profile_image_path.eq(next_profile_image_path),
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
        .await;

    let updated_user = match update_result {
        Ok(user) => user,
        Err(error) => {
            if let Some(uploaded_path) = pending_uploaded_image_path.as_deref() {
                let _ = remove_managed_profile_image(uploaded_path).await;
            }
            return Err(error);
        }
    };

    if updated_user.profile_image_path != existing_profile_image_path {
        if let Some(old_path) = existing_profile_image_path.as_deref() {
            let _ = remove_managed_profile_image(old_path).await;
        }
    }

    Ok(Json(user_summary(updated_user)))
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
    let next_username = form.username.trim().to_string();
    if next_username.is_empty() {
        return Err(Status::BadRequest);
    }

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

    let profile_image_upload = form.profile_image_upload;
    let next_profile_image_path = if let Some(upload) = profile_image_upload {
        Some(store_profile_image(upload).await?)
    } else {
        None
    };

    let user = User {
        id: 0, // This will be auto-incremented by SQLite
        username: next_username,
        password: hashed_password,
        pin: hashed_pin,
        admin: existing_count == 0 || form.admin,
        birthday: form
            .birthday
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        profile_image_path: next_profile_image_path.clone(),
        preferred_metadata_languages_json: serialize_preferred_metadata_languages(
            form.preferred_metadata_languages
                .unwrap_or_else(default_preferred_metadata_languages),
        ),
    };

    // Insert new user
    let insert_result = db
        .run(move |conn| diesel::insert_into(users).values(&user).execute(conn))
        .await;
    if insert_result.is_err() {
        if let Some(uploaded_path) = next_profile_image_path.as_deref() {
            let _ = remove_managed_profile_image(uploaded_path).await;
        }
        return Err(Status::InternalServerError);
    }

    Ok("User created")
}

#[get("/api/v1/user-profile-images/<filename>")]
pub async fn get_user_profile_image(filename: &str) -> Result<NamedFile, Status> {
    if !is_safe_profile_image_filename(filename) {
        return Err(Status::NotFound);
    }

    let root = profile_image_root();
    let image_path = root.join(filename);
    if !image_path.starts_with(&root) {
        return Err(Status::NotFound);
    }

    NamedFile::open(image_path)
        .await
        .map_err(|_| Status::NotFound)
}

async fn store_profile_image(upload: ProfileImageUploadForm) -> Result<String, Status> {
    let (bytes, extension) = validate_profile_image(upload)?;
    let hash = Sha256::digest(&bytes);
    let hash_prefix = hash[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Status::InternalServerError)?
        .as_millis();
    let filename = format!("profile-{timestamp}-{hash_prefix}.{extension}");
    let root = profile_image_root();
    fs::create_dir_all(&root)
        .await
        .map_err(|_| Status::InternalServerError)?;
    let image_path = root.join(&filename);
    fs::write(image_path, bytes)
        .await
        .map_err(|_| Status::InternalServerError)?;
    Ok(filename)
}

fn validate_profile_image(
    upload: ProfileImageUploadForm
) -> Result<(Vec<u8>, &'static str), Status> {
    let declared_mime_type = upload.mime_type.trim().to_ascii_lowercase();
    if !matches!(
        declared_mime_type.as_str(),
        "image/jpeg" | "image/png" | "image/webp" | "image/gif"
    ) {
        return Err(Status::UnsupportedMediaType);
    }

    let data_base64 = upload.data_base64.trim();
    let bytes = general_purpose::STANDARD
        .decode(data_base64)
        .map_err(|_| Status::BadRequest)?;
    if bytes.is_empty() {
        return Err(Status::BadRequest);
    }
    if bytes.len() > PROFILE_IMAGE_MAX_BYTES {
        return Err(Status::PayloadTooLarge);
    }

    let format = image::guess_format(&bytes).map_err(|_| Status::UnsupportedMediaType)?;
    let extension = match format {
        image::ImageFormat::Jpeg => "jpg",
        image::ImageFormat::Png => "png",
        image::ImageFormat::WebP => "webp",
        image::ImageFormat::Gif => "gif",
        _ => return Err(Status::UnsupportedMediaType),
    };

    Ok((bytes, extension))
}

fn profile_image_root() -> PathBuf {
    let env = Environment::from_usize(CURRENT_ENV.load(Ordering::Relaxed));
    let data_dir = match env {
        Environment::Test => PathBuf::from("./test_data"),
        Environment::Production => PathBuf::from(current_settings().general.data_dir),
    };
    data_dir.join("users").join("profile-images")
}

fn user_summary(user: User) -> UserSummary {
    UserSummary {
        id: user.id,
        username: user.username,
        admin: user.admin,
        birthday: user.birthday,
        profile_image_url: user.profile_image_path.and_then(|path| {
            if is_safe_profile_image_filename(&path) {
                Some(format!("{PROFILE_IMAGE_ROUTE_PREFIX}{path}"))
            } else {
                None
            }
        }),
        preferred_metadata_languages: parse_preferred_metadata_languages(
            &user.preferred_metadata_languages_json,
        ),
    }
}

async fn remove_managed_profile_image(path: &str) -> Result<(), Status> {
    if !is_safe_profile_image_filename(path) {
        return Ok(());
    }
    let root = profile_image_root();
    let image_path = root.join(path);
    if image_path.starts_with(&root) {
        let _ = fs::remove_file(image_path).await;
    }
    Ok(())
}

fn is_safe_profile_image_filename(filename: &str) -> bool {
    !filename.is_empty()
        && Path::new(filename)
            .file_name()
            .is_some_and(|file_name| file_name == filename)
        && filename.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
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
