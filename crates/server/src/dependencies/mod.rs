#![doc = "Module for everything related to dependencies."]

use cargo_metadata::{MetadataCommand, Package};
use std::error::Error;

/// Get the dependencies from the Cargo.toml file.
pub fn get_dependencies() -> Result<Vec<Package>, Box<dyn Error>> {
    let metadata = MetadataCommand::new().exec()?;
    Ok(metadata.packages)
}
