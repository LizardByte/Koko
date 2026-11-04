//! Isolated databases and bounded loopback servers for capture and provider regressions.

use super::{
    Channel,
    Source,
    SourceConfig,
    settings,
    storage,
};
use crate::db::{
    DbConn,
    schema::{
        live_tv_channels as c,
        live_tv_sources as s,
        users,
    },
};
use diesel::prelude::*;
use std::sync::{
    Arc,
    Mutex,
    atomic::{
        AtomicI32,
        Ordering,
    },
};
use tokio::io::{
    AsyncReadExt,
    AsyncWriteExt,
};

static NEXT_SOURCE: AtomicI32 = AtomicI32::new(100_000);

pub(super) struct Database {
    pub rocket: rocket::Rocket<rocket::Ignite>,
    pub directory: tempfile::TempDir,
    pub path: String,
}

impl Database {
    pub async fn new() -> Self {
        let (directory, path) = tokio::task::spawn_blocking(|| {
            let directory = tempfile::tempdir().unwrap();
            let path = directory
                .path()
                .join("tv.sqlite")
                .to_string_lossy()
                .into_owned();
            let mut conn = SqliteConnection::establish(&path).unwrap();
            crate::db::configure_sqlite_connection(&mut conn).unwrap();
            crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
            diesel::insert_into(users::table)
                .values((
                    users::id.eq(1),
                    users::username.eq("capture-viewer"),
                    users::password.eq(format!("{:032x}", rand::random::<u128>())),
                ))
                .execute(&mut conn)
                .unwrap();
            settings::save(
                &mut conn,
                &settings::DvrSettings {
                    recording_dir: directory
                        .path()
                        .join("recordings")
                        .to_string_lossy()
                        .into_owned(),
                },
            )
            .unwrap();
            (directory, path)
        })
        .await
        .unwrap();
        let figment = rocket::Config::figment().merge(("workers", 2)).merge((
            "databases",
            serde_json::json!({"sqlite_db":{"url":path,"pool_size":2}}),
        ));
        let rocket = rocket::custom(figment)
            .attach(DbConn::fairing())
            .ignite()
            .await
            .unwrap();
        Self {
            rocket,
            directory,
            path,
        }
    }

    pub async fn conn(&self) -> DbConn {
        DbConn::get_one(&self.rocket).await.unwrap()
    }

    pub async fn source(
        &self,
        kind: &str,
        config: SourceConfig,
        url: &str,
    ) -> (Source, Channel) {
        let id = NEXT_SOURCE.fetch_add(1, Ordering::Relaxed);
        let kind = kind.to_owned();
        let url = url.to_owned();
        self.conn()
            .await
            .run(move |conn| {
                diesel::insert_into(s::table)
                    .values((
                        s::id.eq(id),
                        s::name.eq("Capture fixture"),
                        s::kind.eq(kind),
                        s::config_json.eq(serde_json::to_string(&config).unwrap()),
                        s::capacity.eq(2),
                        s::next_refresh.eq(i64::MAX),
                    ))
                    .execute(conn)
                    .unwrap();
                diesel::insert_into(c::table)
                    .values((
                        c::id.eq(id),
                        c::source_id.eq(id),
                        c::source_key.eq("station"),
                        c::guide_key.eq("station"),
                        c::name.eq("Workshop channel"),
                        c::number.eq("1"),
                        c::group_name.eq("Sports"),
                        c::stream_url.eq(url),
                    ))
                    .execute(conn)
                    .unwrap();
                (
                    storage::source(conn, id).unwrap(),
                    storage::channel(conn, id).unwrap(),
                )
            })
            .await
    }
}

pub(super) type Reply = (u16, &'static str, Vec<u8>);

pub(super) struct HttpFixture {
    pub url: String,
    pub requests: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl HttpFixture {
    pub async fn new(handler: impl Fn(&str) -> Reply + Send + Sync + 'static) -> Self {
        Self::with_gate(handler, None).await
    }

    pub async fn held(
        handler: impl Fn(&str) -> Reply + Send + Sync + 'static
    ) -> (Self, tokio::sync::watch::Sender<bool>) {
        let (sender, receiver) = tokio::sync::watch::channel(false);
        (Self::with_gate(handler, Some(receiver)).await, sender)
    }

    async fn with_gate(
        handler: impl Fn(&str) -> Reply + Send + Sync + 'static,
        gate: Option<tokio::sync::watch::Receiver<bool>>,
    ) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        let handler = Arc::new(handler);
        let task = tokio::spawn(async move {
            let mut clients = tokio::task::JoinSet::new();
            while let Ok((socket, _)) = listener.accept().await {
                clients.spawn(serve_request(
                    socket,
                    handler.clone(),
                    seen.clone(),
                    gate.clone(),
                ));
            }
        });
        Self {
            url,
            requests,
            task,
        }
    }
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<String> {
    let mut request = Vec::new();
    while request.len() <= 16384 {
        let mut chunk = [0; 2048];
        let count = socket.read(&mut chunk).await.ok()?;
        if count == 0 {
            return None;
        }
        request.extend_from_slice(&chunk[..count]);
        if request.windows(4).any(|s| s == b"\r\n\r\n") {
            return Some(String::from_utf8_lossy(&request).into_owned());
        }
    }
    None
}

async fn serve_request(
    mut socket: tokio::net::TcpStream,
    handler: Arc<impl Fn(&str) -> Reply + Send + Sync>,
    seen: Arc<Mutex<Vec<String>>>,
    mut gate: Option<tokio::sync::watch::Receiver<bool>>,
) {
    if let Some(gate) = gate.as_mut() {
        if gate.wait_for(|open| *open).await.is_err() {
            return;
        }
    }
    let Some(request) = read_request(&mut socket).await else {
        return;
    };
    let path = request.split_whitespace().nth(1).unwrap_or("/");
    seen.lock().unwrap().push(path.to_owned());
    let (status, mime, body) = handler(path);
    let header = format!(
        "HTTP/1.1 {status} Fixture\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: \
         close\r\n\r\n",
        body.len()
    );
    if socket.write_all(header.as_bytes()).await.is_ok() {
        let _ = socket.write_all(&body).await;
    }
}

impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
