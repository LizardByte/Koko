//! Routes for the web server.

// standard imports
use std::env;
use std::path::{
    Path,
    PathBuf,
};

// lib imports
use rocket::fs::NamedFile;
use rocket::get;
use rocket::http::uri::{
    Segments,
    fmt::Path as UriPath,
};
use rocket::response::content::RawHtml;

// local imports
use crate::globals;

#[get("/")]
pub async fn index() -> Result<NamedFile, RawHtml<String>> {
    let index_path = web_client_index_path();

    if let Some(index_path) = index_path {
        return NamedFile::open(index_path)
            .await
            .map_err(|_| RawHtml(web_client_missing_html()));
    }

    Err(RawHtml(web_client_missing_html()))
}

#[get("/<path..>", rank = 100)]
pub async fn spa_asset(path: Segments<'_, UriPath>) -> Option<NamedFile> {
    let dist_dir = web_client_dist_dir()?;
    let requested_path = path.to_path_buf(false).ok();

    if let Some(requested_path) = requested_path {
        let requested_path = dist_dir.join(&requested_path);
        if requested_path.is_file() {
            return NamedFile::open(requested_path).await.ok();
        }
    }

    let has_extension = path
        .clone()
        .last()
        .is_some_and(|segment| Path::new(segment).extension().is_some());

    if !has_extension {
        return NamedFile::open(dist_dir.join("index.html")).await.ok();
    }

    None
}

fn web_client_index_path() -> Option<PathBuf> {
    let dist_dir = web_client_dist_dir()?;
    let index_path = dist_dir.join("index.html");
    index_path.is_file().then_some(index_path)
}

fn web_client_dist_dir() -> Option<PathBuf> {
    web_client_dist_candidates()
        .into_iter()
        .find(|candidate| candidate.is_dir())
}

fn web_client_dist_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(path) = env::var("KOKO_WEB_CLIENT_DIST") {
        let path = path.trim();
        if !path.is_empty() {
            candidates.push(PathBuf::from(path));
        }
    }

    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join("crates").join("client-web").join("dist"));
    }

    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("client-web")
            .join("dist"),
    );
    candidates
}

fn web_client_missing_html() -> String {
    format!(
        r#"<!doctype html>
<html lang="en-US">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{0}</title>
    <style>
      body {{
        margin: 0;
        min-height: 100vh;
        display: grid;
        place-items: center;
        background: #111827;
        color: #f9fafb;
        font-family: Inter, Segoe UI, system-ui, sans-serif;
      }}
      main {{
        max-width: 48rem;
        padding: 2rem;
        border-radius: 1rem;
        background: #0f172a;
        border: 1px solid rgba(148, 163, 184, 0.2);
      }}
      code {{
        color: #93c5fd;
      }}
    </style>
  </head>
  <body>
    <main>
      <h1>{0}</h1>
      <p>The web client bundle is not available yet.</p>
      <p>Build <code>crates/client-web</code> and make sure the output exists at
      <code>crates/client-web/dist</code>, or set <code>KOKO_WEB_CLIENT_DIST</code>
      to a built client directory.</p>
    </main>
  </body>
</html>"#,
        globals::GLOBAL_APP_NAME
    )
}
