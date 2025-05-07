#![doc = "Logging utilities for the application."]

// standard imports
use std::io;

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
        let mut msg = message.to_string();
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
        out.finish(format_args!(
            "{} [{}] {}",
            chrono::Local::now().format(self.time_format),
            if remove_ansi {
                record.level().to_string()
            } else {
                self.colors.color(record.level()).to_string()
            },
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

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    let logger = Logger::new()?;
    logger.init()
}
