#![doc = "Routes for the web server."]

// lib imports
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::{ExpressionMethods, SelectableHelper};
use rocket::http::Status;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::{get, post};
use rocket_okapi::{openapi, JsonSchema};

// local imports
use crate::auth::{AdminGuard, Claims};
use crate::db::models::User;
use crate::db::DbConn;

#[derive(Deserialize, JsonSchema)]
pub struct LoginForm {
    username: String,
    password: String,
}

#[derive(Serialize, JsonSchema)]
pub struct TokenResponse {
    token: String,
}

#[openapi(tag = "Auth")]
#[post("/login", format = "json", data = "<login_form>")]
pub async fn login(
    db: DbConn,
    login_form: Json<LoginForm>,
) -> Result<Json<TokenResponse>, Status> {
    use crate::db::schema::users::dsl::*;

    let form = login_form.into_inner();
    println!("Attempting login for user: {}", form.username);

    let user = match db
        .run(move |conn| {
            users
                .filter(username.eq(form.username))
                .select(User::as_select())
                .first::<User>(conn)
        })
        .await
    {
        Ok(user) => user,
        Err(e) => {
            println!("Database error: {}", e);
            return Err(Status::Unauthorized);
        }
    };

    // debug print user info from db
    println!("Found user in db: {:?}", user);

    if !crate::auth::verify_password(&form.password, &user.password) {
        println!("Password verification failed");
        return Err(Status::Unauthorized);
    }

    let token = crate::auth::create_token(&user.id.to_string(), crate::auth::get_jwt_secret());
    Ok(Json(TokenResponse { token }))
}

#[openapi(tag = "Auth")]
#[get("/logout")]
pub fn logout() -> &'static str {
    "Logout Page"
}

#[openapi(tag = "Test Auth")]
#[get("/jwt_test")]
pub fn jwt_test(_claims: Claims) -> &'static str {
    "Protected Page"
}

#[openapi(tag = "Test Auth")]
#[get("/admin_test")]
pub fn admin_test(_admin: AdminGuard) -> &'static str {
    "Admin only content"
}
