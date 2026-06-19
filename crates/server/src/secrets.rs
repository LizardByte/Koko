//! Secret-store helpers for sensitive runtime settings.

// standard imports
use std::collections::HashMap;
use std::sync::Mutex;

// lib imports
use keyring_core::{Entry, Error, set_default_store};
use once_cell::sync::Lazy;

// local imports
use crate::globals::GLOBAL_APP_NAME;

static SECRET_STORE_INIT: Lazy<Mutex<Option<Result<(), String>>>> = Lazy::new(|| Mutex::new(None));

fn initialize_secret_store() -> Result<(), String> {
    let configured_store = std::env::var("KOKO_SECRET_STORE")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    match configured_store.as_str() {
        "" | "native" | "os" => use_native_secret_store(),
        "memory" | "mock" | "sample" => {
            let config = HashMap::from([("persist", "false")]);
            use_sample_secret_store(&config)
        }
        store => use_named_secret_store(store),
    }
    .map_err(|error| format!("Failed to initialize credential store: {error}"))
}

fn use_sample_secret_store(config: &HashMap<&str, &str>) -> keyring_core::Result<()> {
    set_default_store(keyring_core::sample::Store::new_with_configuration(config)?);
    Ok(())
}

#[cfg(feature = "native-secret-store")]
fn use_named_secret_store(store: &str) -> keyring_core::Result<()> {
    keyring::use_named_store(store)
}

#[cfg(not(feature = "native-secret-store"))]
fn use_named_secret_store(store: &str) -> keyring_core::Result<()> {
    Err(Error::NotSupportedByStore(format!(
        "credential store {store:?} is not available in this build"
    )))
}

#[cfg(feature = "native-secret-store")]
fn use_native_secret_store() -> keyring_core::Result<()> {
    #[cfg(target_os = "linux")]
    {
        keyring::use_native_store(true)
    }
    #[cfg(not(target_os = "linux"))]
    {
        keyring::use_native_store(false)
    }
}

#[cfg(not(feature = "native-secret-store"))]
fn use_native_secret_store() -> keyring_core::Result<()> {
    let config = HashMap::from([("persist", "false")]);
    use_sample_secret_store(&config)
}

fn ensure_secret_store() -> Result<(), String> {
    let mut guard = SECRET_STORE_INIT
        .lock()
        .map_err(|_| "Secret-store initialization lock is poisoned.".to_string())?;
    if let Some(result) = guard.as_ref() {
        return result.clone();
    }

    let result = initialize_secret_store();
    *guard = Some(result.clone());
    result
}

fn secret_entry(secret_ref: &str) -> Result<Entry, String> {
    ensure_secret_store()?;
    Entry::new(GLOBAL_APP_NAME, secret_ref)
        .map_err(|error| format!("Failed to open credential entry {secret_ref:?}: {error}"))
}

pub(crate) fn store_secret(
    secret_ref: &str,
    value: &str,
) -> Result<(), String> {
    secret_entry(secret_ref)?
        .set_password(value)
        .map_err(|error| format!("Failed to store credential {secret_ref:?}: {error}"))
}

pub(crate) fn load_secret(secret_ref: &str) -> Result<Option<String>, String> {
    match secret_entry(secret_ref)?.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(format!("Failed to load credential {secret_ref:?}: {error}")),
    }
}

pub(crate) fn delete_secret(secret_ref: &str) -> Result<(), String> {
    match secret_entry(secret_ref)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "Failed to delete credential {secret_ref:?}: {error}"
        )),
    }
}
