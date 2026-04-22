//! Logging utilities for the application.

// standard imports
use std::io;
use std::path::Path;

// lib imports
use fern::colors::{Color, ColoredLevelConfig};
use log::LevelFilter;
use regex::Regex;

// local imports
use crate::globals;

#[derive(Clone)]
struct Logger {
    time_format: &'static str,
    replace_str: &'static str,
    colors: ColoredLevelConfig,
    ansi_escape: Regex,
    sensitive_data_patterns: Vec<Regex>,
}

impl Logger {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            time_format: "%Y-%m-%dT%H:%M:%S%.3f%:z",
            replace_str: "***",
            colors: ColoredLevelConfig::new()
                .error(Color::Red)
                .warn(Color::Yellow)
                .info(Color::Green)
                .debug(Color::Blue)
                .trace(Color::Magenta),
            ansi_escape: Regex::new(r"\x1b\[[0-9;]*m")?,
            sensitive_data_patterns: vec![
                Regex::new(r#"password=([^&]+)"#)?,
                Regex::new(r#"token=([^&]+)"#)?,
            ],
        })
    }

    fn format_message(
        &self,
        message: &str,
    ) -> String {
        let mut msg = message.replace("\r\n", " ↩ ").replace(['\n', '\r'], " ↩ ");
        for pattern in &self.sensitive_data_patterns {
            msg = pattern.replace_all(&msg, self.replace_str).to_string();
        }
        msg
    }

    fn format(
        &self,
        out: fern::FormatCallback,
        message: &str,
        record: &log::Record,
        remove_ansi: bool,
    ) {
        let mut msg = self.format_message(message);
        if remove_ansi {
            msg = self.ansi_escape.replace_all(&msg, "").to_string();
        }
        let module = record.module_path().unwrap_or(record.target());
        let file = normalize_log_source_path(record.file().unwrap_or("unknown"));
        let line = record.line().map(|value| value.to_string()).unwrap_or_else(|| "?".into());
        out.finish(format_args!(
            "{} [{}] [{}] [{}:{}] {}",
            chrono::Local::now().format(self.time_format),
            if remove_ansi {
                record.level().to_string()
            } else {
                self.colors.color(record.level()).to_string()
            },
            module,
            file,
            line,
            msg
        ));
    }

    fn configure_dispatch(
        &self,
        to_file: bool,
    ) -> Result<fern::Dispatch, Box<dyn std::error::Error>> {
        let logger = self.clone();
        let dispatch = fern::Dispatch::new().format(move |out, message, record| {
            logger.format(out, &message.to_string(), record, to_file)
        });

        if to_file {
            Ok(dispatch.chain(fern::log_file(globals::APP_PATHS.log_path.clone())?))
        } else {
            Ok(dispatch.chain(io::stdout()))
        }
    }

    fn init(&self) -> Result<(), Box<dyn std::error::Error>> {
        let base_config = fern::Dispatch::new().level(LevelFilter::Debug);

        base_config
            .chain(self.configure_dispatch(false)?)
            .chain(self.configure_dispatch(true)?)
            .apply()?;
        Ok(())
    }
}

fn normalize_path_separators(path: &str) -> String {
    path.trim().replace('\\', "/")
}

fn shorten_absolute_path(path: &str, segments: usize) -> String {
    let parts = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if parts.len() <= segments {
        return parts.join("/");
    }

    parts[parts.len().saturating_sub(segments)..].join("/")
}

fn workspace_relative_path(path: &str) -> Option<String> {
    let manifest_dir = normalize_path_separators(env!("CARGO_MANIFEST_DIR"));
    let manifest_path = Path::new(&manifest_dir);
    let repo_root = manifest_path
        .parent()
        .and_then(Path::parent)
        .map(|path| normalize_path_separators(&path.to_string_lossy()))?;

    [repo_root, manifest_dir]
        .into_iter()
        .find_map(|prefix| {
            let normalized_prefix = prefix.trim_end_matches('/');
            path.strip_prefix(&format!("{normalized_prefix}/"))
                .map(|value| value.to_string())
                .or_else(|| (path == normalized_prefix).then(String::new))
        })
}

pub fn normalize_display_path(path: &str) -> String {
    normalize_path_separators(path)
}

pub fn normalize_log_source_path(path: &str) -> String {
    let normalized = normalize_path_separators(path);
    if normalized.is_empty() {
        return "unknown".into();
    }

    if let Some(relative) = workspace_relative_path(&normalized) {
        return relative;
    }

    if let Some((_, remainder)) = normalized.split_once("/.cargo/registry/src/") {
        if let Some((_, crate_relative)) = remainder.split_once('/') {
            return crate_relative.to_string();
        }
    }

    if let Some((_, remainder)) = normalized.split_once("/cargo/registry/src/") {
        if let Some((_, crate_relative)) = remainder.split_once('/') {
            return crate_relative.to_string();
        }
    }

    if normalized.contains(":/") || normalized.starts_with('/') {
        return shorten_absolute_path(&normalized, 4);
    }

    normalized
}

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let logger = Logger::new()?;
    logger.init()
}
