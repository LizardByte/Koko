//! Shared test utilities to eliminate code duplication across test files.

// standard imports
use std::sync::atomic::{AtomicU64, Ordering};

// lib imports
use rocket::http::{ContentType, Header, Status};
use rocket::local::asynchronous::Client;
use serde_json::Value;

// local imports
use koko::web;

// Global counter to ensure unique database files across all tests
static GLOBAL_TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Enhanced test response structure with headers
pub struct TestResponse {
    pub status: Status,
    pub body: String,
    pub headers: Vec<Header<'static>>,
}

/// Create a test client with an isolated database
pub async fn create_test_client(prefix: Option<&str>) -> Client {
    // Set the test environment first
    use koko::globals::CURRENT_ENV;
    CURRENT_ENV.store(1, Ordering::SeqCst);

    // Use provided prefix or default to "test"
    let prefix = prefix.unwrap_or("test");

    // Create a unique database name for this test
    let test_id = GLOBAL_TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let db_name = format!("{}_{}_{}.db", prefix, test_id, timestamp);

    // Ensure test_data directory exists
    std::fs::create_dir_all("./test_data").expect("Failed to create test_data directory");

    // Create the full database path
    let db_path = format!("./test_data/{}", db_name);

    // Remove the database file if it exists from a previous run
    if std::path::Path::new(&db_path).exists() {
        std::fs::remove_file(&db_path).ok();
    }

    // Create a new rocket instance with the unique database path
    let rocket = web::rocket_with_db_path(Some(db_path));
    let client = Client::tracked(rocket)
        .await
        .expect("Failed to create test client");

    client
}

/// Make an HTTP request to the Rocket application.
/// Returns either a simple tuple (Status, String) or enhanced TestResponse based on return_headers
pub async fn make_request(
    client: Option<&Client>,
    method: &str,
    path: &str,
    json_body: Option<Value>,
    auth_header: Option<String>,
    expected_status: Option<Status>,
    return_headers: Option<bool>,
) -> TestResponse {
    let owned_client;
    let client = match client {
        Some(c) => c,
        None => {
            let rocket = web::rocket();
            owned_client = Client::tracked(rocket)
                .await
                .expect("Failed to launch web server");
            &owned_client
        }
    };

    let mut request = match method.to_lowercase().as_str() {
        "get" => client.get(path),
        "post" => client.post(path),
        "put" => client.put(path),
        "delete" => client.delete(path),
        "patch" => client.patch(path),
        _ => panic!("Unsupported HTTP method: {}", method),
    };

    if let Some(json) = json_body {
        request = request.header(ContentType::JSON).body(json.to_string());
    }

    if let Some(auth) = auth_header {
        request = request.header(Header::new("Authorization", auth));
    }

    let response = request.dispatch().await;
    let status = response.status();

    // Assert expected status if provided
    if let Some(expected) = expected_status {
        assert_eq!(
            status, expected,
            "Expected status {} but got {}",
            expected, status
        );
    }

    let headers = if return_headers.unwrap_or(false) {
        response
            .headers()
            .iter()
            .map(|h| Header::new(h.name().to_string(), h.value().to_string()))
            .collect()
    } else {
        Vec::new()
    };

    let body = response.into_string().await.unwrap_or_default();

    TestResponse {
        status,
        body,
        headers,
    }
}

/// Create a user via the API (useful for test setup)
pub async fn create_test_user(
    client: &Client,
    username: &str,
    password: &str,
    admin: bool,
    pin: Option<&str>,
    expected_status: Option<Status>,
) -> (Status, String) {
    use serde_json::json;

    let mut user_data = json!({
        "username": username,
        "password": password,
        "admin": admin
    });

    if let Some(pin_value) = pin {
        user_data
            .as_object_mut()
            .unwrap()
            .insert("pin".to_string(), json!(pin_value));
    }

    let response = make_request(
        Some(client),
        "post",
        "/create_user",
        Some(user_data),
        None,
        expected_status,
        Some(false),
    )
    .await;
    (response.status, response.body)
}

/// Login a user and return the token
pub async fn login_user(
    client: &Client,
    username: &str,
    password: &str,
    expected_status: Option<Status>,
) -> Result<String, String> {
    use serde_json::json;

    let login_data = json!({
        "username": username,
        "password": password
    });

    let response = make_request(
        Some(client),
        "post",
        "/login",
        Some(login_data),
        None,
        expected_status,
        Some(false),
    )
    .await;

    if response.status == Status::Ok {
        let response_json: Value = serde_json::from_str(&response.body)
            .map_err(|e| format!("Failed to parse login response: {}", e))?;

        response_json["token"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Token not found in response".to_string())
    } else {
        Err(format!(
            "Login failed with status: {} - {}",
            response.status, response.body
        ))
    }
}

/// Create a user and login, returning the token
pub async fn create_and_login_user(
    client: &Client,
    username: &str,
    password: &str,
    admin: bool,
    pin: Option<&str>,
) -> Result<String, String> {
    let (create_status, create_body) =
        create_test_user(client, username, password, admin, pin, Some(Status::Ok)).await;

    if create_status != Status::Ok {
        return Err(format!(
            "Failed to create user: {} - {}",
            create_status, create_body
        ));
    }

    login_user(client, username, password, Some(Status::Ok)).await
}
