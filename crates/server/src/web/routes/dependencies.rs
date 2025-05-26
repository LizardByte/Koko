#![doc = "Routes for dependencies."]

// lib imports
use cargo_metadata::Package;
use rocket::get;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket_okapi::openapi;
use schemars::JsonSchema;
use serde::Serialize;

// local imports
use crate::dependencies;

// Create your own response type that derives Serialize and JsonSchema.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PackageResponse {
    pub name: String,
    pub version: String,
    pub license: Option<String>,
}

// Implement conversion from the external Package type to your own PackageResponse.
impl From<Package> for PackageResponse {
    fn from(pkg: Package) -> Self {
        PackageResponse {
            name: pkg.name,
            version: pkg.version.to_string(),
            license: pkg.license,
        }
    }
}

#[openapi(tag = "Dependencies")]
#[get("/dependencies")]
pub fn get_dependencies() -> Result<Json<Vec<PackageResponse>>, Status> {
    let deps = dependencies::get_dependencies().map_err(|e| {
        log::error!("Failed to get dependencies: {}", e);
        Status::InternalServerError
    })?;
    let response = deps.into_iter().map(PackageResponse::from).collect();
    Ok(Json(response))
}
