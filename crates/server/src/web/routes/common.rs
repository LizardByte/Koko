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
use rocket::http::{
    ContentType,
    Status,
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

/// Serve the application icon even when the web-client bundle is unavailable.
#[get("/favicon.ico")]
pub fn favicon() -> (ContentType, &'static [u8]) {
    (
        ContentType::Icon,
        include_bytes!("../../../../../assets/icon.ico"),
    )
}

/// Serve only the provider logos embedded in the backend, independently of the web-client bundle.
#[get("/assets/providers/<filename>")]
pub fn provider_logo(filename: &str) -> Result<(ContentType, &'static [u8]), Status> {
    let asset: (ContentType, &'static [u8]) = match filename {
        "audiosilo.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/audiosilo.svg"),
        ),
        "comicvine.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/comicvine.svg"),
        ),
        "fanart.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/fanart.png"),
        ),
        "fastchannels.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/fastchannels.svg"),
        ),
        "hdhomerun.ico" => (
            ContentType::Icon,
            include_bytes!("../../../../../assets/providers/hdhomerun.ico"),
        ),
        "libex-dark.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/libex-dark.png"),
        ),
        "libex-light.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/libex-light.png"),
        ),
        "librivox.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/librivox.png"),
        ),
        "local-nfo.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/local-nfo.svg"),
        ),
        "lrclib.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/lrclib.png"),
        ),
        "musicbrainz.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/musicbrainz.svg"),
        ),
        "nextpvr.ico" => (
            ContentType::Icon,
            include_bytes!("../../../../../assets/providers/nextpvr.ico"),
        ),
        "open-library.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/open-library.png"),
        ),
        "themerr.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/themerr.png"),
        ),
        "tmdb.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/tmdb.svg"),
        ),
        "trailerdb-dark.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/trailerdb-dark.svg"),
        ),
        "trailerdb-light.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/trailerdb-light.svg"),
        ),
        "tv-adultswim.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-adultswim.png"),
        ),
        "tv-amazon.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/tv-amazon.svg"),
        ),
        "tv-bally.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/tv-bally.svg"),
        ),
        "tv-cbs.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-cbs.png"),
        ),
        "tv-espn.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-espn.png"),
        ),
        "tv-foxone.ico" => (
            ContentType::Icon,
            include_bytes!("../../../../../assets/providers/tv-foxone.ico"),
        ),
        "tv-freelivesports.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-freelivesports.png"),
        ),
        "tv-fubo.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-fubo.png"),
        ),
        "tv-hallmark.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-hallmark.png"),
        ),
        "tv-hulu.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-hulu.png"),
        ),
        "tv-lg.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/tv-lg.svg"),
        ),
        "tv-localnow.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-localnow.png"),
        ),
        "tv-nfl.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-nfl.png"),
        ),
        "tv-plex.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/tv-plex.svg"),
        ),
        "tv-pluto.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-pluto.png"),
        ),
        "tv-reelz.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-reelz.png"),
        ),
        "tv-roku.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/tv-roku.svg"),
        ),
        "tv-samsung.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/tv-samsung.svg"),
        ),
        "tv-sling.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/tv-sling.svg"),
        ),
        "tv-spectrum.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-spectrum.png"),
        ),
        "tv-stirr.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-stirr.png"),
        ),
        "tv-tcl.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-tcl.png"),
        ),
        "tv-tubi.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-tubi.png"),
        ),
        "tv-vizio.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-vizio.png"),
        ),
        "tv-whale.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/tv-whale.svg"),
        ),
        "tv-wsn.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-wsn.png"),
        ),
        "tv-xumo.svg" => (
            ContentType::SVG,
            include_bytes!("../../../../../assets/providers/tv-xumo.svg"),
        ),
        "tv-youtube.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tv-youtube.png"),
        ),
        "tvdb-dark.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tvdb-dark.png"),
        ),
        "tvdb-light.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tvdb-light.png"),
        ),
        "tvheadend.png" => (
            ContentType::PNG,
            include_bytes!("../../../../../assets/providers/tvheadend.png"),
        ),
        _ => return Err(Status::NotFound),
    };
    Ok(asset)
}

#[get("/<path..>", rank = 100)]
pub async fn spa_asset(path: Segments<'_, UriPath>) -> Option<NamedFile> {
    if path.clone().next() == Some("api") {
        return None;
    }
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

    if let Ok(executable_path) = env::current_exe() {
        if let Some(executable_dir) = executable_path.parent() {
            candidates.push(
                executable_dir
                    .join("crates")
                    .join("client-web")
                    .join("dist"),
            );

            #[cfg(any(target_os = "linux", target_os = "macos"))]
            {
                for ancestor in executable_dir.ancestors() {
                    #[cfg(target_os = "macos")]
                    candidates.push(ancestor.join("Resources").join("client-web").join("dist"));
                    #[cfg(target_os = "linux")]
                    candidates.push(ancestor.join("share").join("koko").join("client-web"));
                }
            }
        }
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
    <link rel="icon" type="image/x-icon" href="/favicon.ico" />
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
