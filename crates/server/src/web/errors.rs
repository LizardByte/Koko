//! Consistent, user-facing JSON errors for API clients.
use rocket::{
    Request,
    Response,
    http::ContentType,
    tokio::io::AsyncReadExt,
};
use serde::Serialize;

#[derive(Serialize)]
struct ApiError {
    status: u16,
    message: String,
}

fn message(
    status: u16,
    body: &str,
) -> String {
    let known = match status {
        401 => "Your session has expired. Sign in again.",
        403 => "You do not have permission to do this.",
        404 => "This item is unavailable or you no longer have access to it.",
        408 => "The request took too long. Please try again.",
        409 => "This conflicts with another change. Refresh and try again.",
        413 => "This request is too large. Try selecting fewer items.",
        429 => "Too many requests. Wait a moment and try again.",
        500 => "The server could not complete this request. Please try again.",
        502 => "An external service is temporarily unavailable. Please try again.",
        503 => "The service is temporarily unavailable. Please try again shortly.",
        504 => "The service took too long to respond. Please try again.",
        400 | 422 => return validation_message(body),
        _ => "The request could not be completed. Please try again.",
    };
    known.to_owned()
}

fn validation_message(body: &str) -> String {
    let parsed = serde_json::from_str::<serde_json::Value>(body).ok();
    let detail = parsed
        .as_ref()
        .and_then(|value| value.as_str().or_else(|| value.get("message")?.as_str()))
        .unwrap_or(body)
        .trim();
    if !detail.is_empty()
        && detail.len() <= 240
        && !detail.contains(['<', '>', '\n', '\r', '{', '}'])
    {
        detail.to_owned()
    } else {
        "Some information is missing or invalid. Check your entries and try again.".to_owned()
    }
}

pub(super) async fn normalize(
    request: &Request<'_>,
    response: &mut Response<'_>,
) {
    let path = request.uri().path();
    if response.status().code < 400
        || !(path.starts_with("/api/")
            || matches!(
                path.as_str(),
                "/login" | "/logout" | "/create_user" | "/user_info" | "/jwt_test" | "/admin_test"
            ))
    {
        return;
    }
    let status = response.status().code;
    let mut body = String::new();
    let podcast_error = status == 502 && path.starts_with("/api/v1/podcasts");
    if matches!(status, 400 | 422) || podcast_error {
        let _ = AsyncReadExt::take(response.body_mut(), 1024)
            .read_to_string(&mut body)
            .await;
    }
    let payload = ApiError {
        status,
        message: podcast_message(podcast_error, &body).unwrap_or_else(|| message(status, &body)),
    };
    let json = serde_json::to_string(&payload).expect("API error contains only scalar fields");
    response.set_header(ContentType::JSON);
    response.set_sized_body(json.len(), std::io::Cursor::new(json));
}

fn podcast_message(
    allowed: bool,
    body: &str,
) -> Option<String> {
    if !allowed {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    (value.get("kind")?.as_str()? == "podcast_operation").then(|| validation_message(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicit_podcast_operation_errors_keep_their_public_detail() {
        let body = r#"{"kind":"podcast_operation","message":"Podcast response is too large"}"#;
        assert_eq!(
            podcast_message(true, body).as_deref(),
            Some("Podcast response is too large")
        );
        assert!(podcast_message(false, body).is_none());
        assert!(podcast_message(true, r#"{"message":"SQL internal failure"}"#).is_none());
        assert!(podcast_message(true, "<html>proxy failure</html>").is_none());
    }

    #[test]
    fn errors_hide_internal_responses_and_preserve_concise_validation() {
        for status in [
            401, 403, 404, 408, 409, 413, 429, 500, 502, 503, 504,
        ] {
            let text = message(status, "<html>SQL error /private/server</html>");
            assert!(
                !text.contains(['<', '>']) && !text.contains("private") && !text.contains("SQL")
            );
        }
        assert_eq!(
            message(422, r#"{"message":"Playlist name is required."}"#),
            "Playlist name is required."
        );
        assert!(message(400, "<html>error</html>").contains("Check your entries"));
    }
}
