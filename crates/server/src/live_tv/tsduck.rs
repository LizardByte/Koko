//! Owned native tuner handles and broadcast decoding through Koko's bundled TSDuck bridge.

use super::{
    Channel,
    Source,
    SourceConfig,
};
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::Serialize;
use std::{
    ffi::{
        CStr,
        CString,
        c_char,
        c_void,
    },
    path::PathBuf,
    sync::Arc,
};
use tokio::io::AsyncWriteExt;

type NativeString = unsafe extern "C" fn() -> *mut c_char;
type Free = unsafe extern "C" fn(*mut c_char);
type Open = unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_void;
type Read = unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> usize;
type Close = unsafe extern "C" fn(*mut c_void);
type Decode = unsafe extern "C" fn(*const u8, usize) -> *mut c_char;
type Scan = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    *const c_char,
    *const c_char,
    unsafe extern "C" fn(*mut c_void) -> bool,
    *mut c_void,
) -> *mut c_char;

struct Api {
    _library: libloading::Library,
    devices: NativeString,
    free: Free,
    open: Open,
    read: Read,
    close: Close,
    decode: Decode,
    scan: Scan,
}

fn library_path() -> Result<PathBuf, String> {
    let filename = if cfg!(windows) {
        "koko_tv.dll"
    } else if cfg!(target_os = "macos") {
        "libkoko_tv.dylib"
    } else {
        "libkoko_tv.so"
    };
    if let Some(root) = std::env::var_os("KOKO_TSDUCK_ROOT") {
        return Ok(PathBuf::from(root).join(filename));
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("live-tv").join(filename)))
        .ok_or_else(|| "Unable to locate Koko's native TV component".into())
}

fn api() -> Result<Arc<Api>, String> {
    static API: once_cell::sync::OnceCell<Result<Arc<Api>, String>> =
        once_cell::sync::OnceCell::new();
    API.get_or_init(|| {
        let path = library_path()?;
        if let Some(directory) = path.parent() {
            let mut paths = vec![directory.to_path_buf()];
            #[cfg(target_os = "macos")]
            if let Some(contents) = directory.parent().and_then(std::path::Path::parent) {
                let resources = contents.join("Resources/live-tv");
                if resources.is_dir() {
                    paths.push(resources);
                }
            }
            if let Some(existing) = std::env::var_os("TSPLUGINS_PATH") {
                paths.extend(std::env::split_paths(&existing));
            }
            std::env::set_var(
                "TSPLUGINS_PATH",
                std::env::join_paths(paths).map_err(|_| "Invalid native TV component path")?,
            );
        }
        // SAFETY: only load the packaged component at an absolute, server-controlled path.
        unsafe {
            #[cfg(windows)]
            let library: libloading::Library =
                libloading::os::windows::Library::load_with_flags(&path, 0x1100)
                    .map_err(|_| {
                        "Koko's native TV component is missing or could not be loaded; reinstall \
                         the complete Koko package"
                    })?
                    .into();
            #[cfg(not(windows))]
            let library = libloading::Library::new(&path).map_err(|_| {
                "Koko's native TV component is missing or could not be loaded; reinstall the \
                 complete Koko package"
            })?;
            let abi: unsafe extern "C" fn() -> u32 = *library
                .get(b"koko_tv_abi\0")
                .map_err(|_| "Invalid native TV component")?;
            if abi() != 1 {
                return Err("The native TV component does not match this Koko version".into());
            }
            let result = Api {
                devices: *library
                    .get(b"koko_tv_devices\0")
                    .map_err(|_| "Invalid native TV component")?,
                free: *library
                    .get(b"koko_tv_free\0")
                    .map_err(|_| "Invalid native TV component")?,
                open: *library
                    .get(b"koko_tv_open\0")
                    .map_err(|_| "Invalid native TV component")?,
                read: *library
                    .get(b"koko_tv_read\0")
                    .map_err(|_| "Invalid native TV component")?,
                close: *library
                    .get(b"koko_tv_close\0")
                    .map_err(|_| "Invalid native TV component")?,
                decode: *library
                    .get(b"koko_tv_decode\0")
                    .map_err(|_| "Invalid native TV component")?,
                scan: *library
                    .get(b"koko_tv_scan\0")
                    .map_err(|_| "Invalid native TV component")?,
                _library: library,
            };
            Ok(Arc::new(result))
        }
    })
    .clone()
}

impl Api {
    // SAFETY: value is null or a NUL-terminated allocation returned by this exact library.
    unsafe fn string(
        &self,
        value: *mut c_char,
    ) -> Result<String, String> {
        if value.is_null() {
            return Err(
                "Native TV operation failed; check the tuner driver, reception, and device \
                 permissions"
                    .into(),
            );
        }
        let result = unsafe { CStr::from_ptr(value) }
            .to_str()
            .map(str::to_owned)
            .map_err(|_| "The native TV component returned invalid text".into());
        unsafe {
            (self.free)(value);
        }
        result
    }
}

fn cstring(value: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| "Tuner settings contain an invalid character".into())
}

#[derive(Serialize, JsonSchema)]
pub(crate) struct Device {
    pub id: String,
    pub name: String,
    pub info: String,
}

pub(crate) async fn devices() -> Result<Vec<Device>, String> {
    let api = api()?;
    let value = tokio::task::spawn_blocking(move || unsafe { api.string((api.devices)()) })
        .await
        .map_err(|_| "Tuner discovery stopped")??;
    let doc = roxmltree::Document::parse(&value).map_err(|_| "Invalid tuner discovery response")?;
    Ok(doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("tuner"))
        .map(|n| Device {
            id: n.attribute("id").unwrap_or("").into(),
            name: n.attribute("name").unwrap_or("").into(),
            info: n.attribute("info").unwrap_or("").into(),
        })
        .collect())
}

pub(crate) async fn rescan(
    db: &crate::db::DbConn,
    id: i32,
) -> Result<(), String> {
    use crate::db::schema::live_tv_sources as s;
    use diesel::prelude::*;
    let lock = super::sources::source_lock(id);
    let _guard = lock.lock().await;
    let source = db
        .run(move |conn| super::storage::source(conn, id))
        .await
        .map_err(super::db_error)?;
    if source.kind != "native" {
        return Err("Channel scanning is available for local tuner sources".into());
    }
    let config = super::sources::config(&source)?;
    match tokio::fs::remove_file(cache(id, &config)).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("Unable to prepare a new channel scan".into()),
    }
    db.run(move |conn| {
        diesel::update(s::table.find(id))
            .set(s::next_refresh.eq(0_i64))
            .execute(conn)
    })
    .await
    .map_err(super::db_error)?;
    Ok(())
}

pub(super) async fn decode(bytes: Vec<u8>) -> Result<super::formats::Guide, String> {
    let api = api()?;
    let value = tokio::task::spawn_blocking(move || unsafe {
        api.string((api.decode)(bytes.as_ptr(), bytes.len()))
    })
    .await
    .map_err(|_| "The broadcast decoder stopped")??;
    super::broadcast::parse(&value)
}

pub(super) fn parse_channels(value: &str) -> Result<Vec<Channel>, String> {
    let doc = roxmltree::Document::parse_with_options(
        value,
        roxmltree::ParsingOptions {
            nodes_limit: 100_000,
            ..Default::default()
        },
    )
    .map_err(|_| "Invalid native tuner channel list")?;
    let mut channels = Vec::new();
    for node in doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("channel"))
    {
        let (Some(service), Some(frequency), Some(tuning)) = (
            node.attribute("id"),
            node.attribute("frequency"),
            node.attribute("tuning"),
        ) else {
            continue;
        };
        let service = service
            .parse::<u16>()
            .map_err(|_| "Invalid native tuner service")?;
        let frequency = frequency
            .parse::<u64>()
            .map_err(|_| "Invalid native tuner frequency")?;
        let key = format!("{frequency}:{service}");
        let mut channel = Channel::new(
            key,
            node.attribute("name")
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Service {service}")),
            tuning.into(),
        );
        channel.number = node.attribute("number").unwrap_or("").into();
        channels.push(channel);
    }
    if channels.is_empty() {
        return Err(
            "No unencrypted channels were found; check the antenna, region, and delivery system, \
             then scan again"
                .into(),
        );
    }
    Ok(channels)
}

fn cache(
    id: i32,
    config: &SourceConfig,
) -> PathBuf {
    use sha2::{
        Digest,
        Sha256,
    };
    let key = serde_json::to_vec(&(
        &config.native_device,
        &config.delivery_system,
        &config.region,
        &config.tuning_options,
    ))
    .unwrap();
    let fingerprint = Sha256::digest(key)
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    PathBuf::from(crate::config::current_settings().general.data_dir)
        .join("live-tv")
        .join(format!("native-{id}-{}.xml", &fingerprint[..16]))
}

pub(super) async fn channels(
    source: &Source,
    config: &SourceConfig,
) -> Result<Vec<Channel>, String> {
    let path = cache(source.id, config);
    if let Ok(file) = tokio::fs::File::open(&path).await {
        use tokio::io::AsyncReadExt;
        let mut value = String::new();
        file.take(16 * 1024 * 1024 + 1)
            .read_to_string(&mut value)
            .await
            .map_err(|_| "Unable to read the tuner channel list")?;
        if value.len() > 16 * 1024 * 1024 {
            return Err("The native tuner channel list exceeds 16 MiB".into());
        }
        return parse_channels(&value);
    }
    scan_channels(source, config).await
}

pub(super) async fn scan_channels(
    source: &Source,
    config: &SourceConfig,
) -> Result<Vec<Channel>, String> {
    let path = cache(source.id, config);
    let api = api()?;
    let _lease = super::stream::lease(source)?;
    let (_scan, cancel) = super::broadcast::scan(source.id);
    let device = cstring(&config.native_device)?;
    let system =
        cstring(if config.delivery_system.is_empty() { "ATSC" } else { &config.delivery_system })?;
    let region = cstring(if config.region.is_empty() { "north-america" } else { &config.region })?;
    let extra = cstring(&config.tuning_options)?;
    struct Cancellation {
        stop: Arc<std::sync::atomic::AtomicBool>,
        receiver: tokio::sync::watch::Receiver<bool>,
    }
    struct CancelOnDrop(Arc<std::sync::atomic::AtomicBool>);
    impl Drop for CancelOnDrop {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let _cancel_on_drop = CancelOnDrop(stop.clone());
    unsafe extern "C" fn cancelled(context: *mut c_void) -> bool {
        let context = unsafe { &*context.cast::<Cancellation>() };
        context.stop.load(std::sync::atomic::Ordering::Relaxed)
            || *context.receiver.borrow()
            || context.receiver.has_changed().is_err()
    }
    let value = tokio::task::spawn_blocking(move || {
        // Keep the scan lease and cancellation receiver alive until native code has closed the
        // tuner.
        let _lease = _lease;
        let _scan = _scan;
        let mut cancel = Cancellation {
            stop,
            receiver: cancel,
        };
        unsafe {
            api.string((api.scan)(
                device.as_ptr(),
                system.as_ptr(),
                region.as_ptr(),
                extra.as_ptr(),
                cancelled,
                (&mut cancel as *mut Cancellation).cast(),
            ))
        }
    })
    .await
    .map_err(|_| "The channel scan stopped")??;
    let result = parse_channels(&value)?;
    tokio::fs::create_dir_all(path.parent().unwrap())
        .await
        .map_err(|_| "Unable to save the tuner channel scan")?;
    tokio::fs::write(&path, value)
        .await
        .map_err(|_| "Unable to save the tuner channel scan")?;
    Ok(result)
}

/// The receiver and COM apartment are created, read, and destroyed on one blocking worker.
pub(super) async fn tune(
    config: &SourceConfig,
    channel: &Channel,
) -> Result<tokio::io::DuplexStream, String> {
    let api = api()?;
    let device = cstring(&config.native_device)?;
    let options = cstring(&channel.stream_url)?;
    let (sender, mut receiver) = tokio::sync::mpsc::channel::<Vec<u8>>(4);
    let (ready, started) = tokio::sync::oneshot::channel();
    tokio::task::spawn_blocking(move || {
        // SAFETY: the handle is confined to this worker; all buffers and C strings outlive calls.
        unsafe {
            let handle = (api.open)(device.as_ptr(), options.as_ptr());
            if handle.is_null() {
                let _ = ready.send(false);
                return;
            }
            struct Handle(*mut c_void, Arc<Api>);
            impl Drop for Handle {
                fn drop(&mut self) {
                    unsafe {
                        (self.1.close)(self.0);
                    }
                }
            }
            let _handle = Handle(handle, api.clone());
            if ready.send(true).is_err() {
                return;
            }
            while !sender.is_closed() {
                let mut bytes = vec![0; 188 * 256];
                let length = (api.read)(handle, bytes.as_mut_ptr(), 256);
                if length == 0 || length > bytes.len() {
                    break;
                }
                bytes.truncate(length);
                if sender.blocking_send(bytes).is_err() {
                    break;
                }
            }
        }
    });
    if !started.await.unwrap_or(false) {
        return Err(
            "Unable to tune this channel; check reception and that another application is not \
             using the tuner"
                .into(),
        );
    }
    let (mut writer, reader) = tokio::io::duplex(188 * 1024);
    tokio::spawn(async move {
        while let Some(bytes) = receiver.recv().await {
            if writer.write_all(&bytes).await.is_err() {
                break;
            }
        }
    });
    Ok(reader)
}
