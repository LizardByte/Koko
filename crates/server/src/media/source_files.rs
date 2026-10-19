//! Bounded, scalar-only source inspection shared by every catalog item type.

#[cfg(test)]
mod tests;

use diesel::{
    ExpressionMethods,
    OptionalExtension,
    QueryDsl,
    RunQueryDsl,
    SqliteConnection,
    sql_types,
};
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::Serialize;

/// One physical file belonging to an item or any of its descendants.
#[derive(Debug, Serialize, JsonSchema, diesel::QueryableByName)]
pub struct MediaItemSourceFile {
    /// Stable library file membership identifier.
    #[diesel(sql_type = sql_types::Integer)]
    pub id: i32,
    /// Catalog item backed by this file.
    #[diesel(sql_type = sql_types::Integer)]
    pub item_id: i32,
    /// Catalog item type, including future types.
    #[diesel(sql_type = sql_types::Text)]
    pub item_type: String,
    /// Local scanner title before provider presentation overrides.
    #[diesel(sql_type = sql_types::Text)]
    pub display_title: String,
    /// Full physical path on the server.
    #[diesel(sql_type = sql_types::Text)]
    pub path: String,
    /// Library root that discovered the file.
    #[diesel(sql_type = sql_types::Text)]
    pub source_root_path: String,
    /// Path relative to the library root.
    #[diesel(sql_type = sql_types::Text)]
    pub relative_path: String,
    /// Physical file size in bytes.
    #[diesel(sql_type = sql_types::BigInt)]
    pub file_size: i64,
    /// File modification time in Unix seconds.
    #[diesel(sql_type = sql_types::Nullable<sql_types::BigInt>)]
    pub modified_at: Option<i64>,
    /// Time the scanner first noticed this file was missing.
    #[diesel(sql_type = sql_types::Nullable<sql_types::BigInt>)]
    pub missing_since: Option<i64>,
    /// Probed container name.
    #[diesel(sql_type = sql_types::Nullable<sql_types::Text>)]
    pub container: Option<String>,
    /// Probed duration in milliseconds.
    #[diesel(sql_type = sql_types::Nullable<sql_types::BigInt>)]
    pub duration_ms: Option<i64>,
    /// Probed bitrate in bits per second.
    #[diesel(sql_type = sql_types::Nullable<sql_types::BigInt>)]
    pub bit_rate: Option<i64>,
    /// Video width in pixels.
    #[diesel(sql_type = sql_types::Nullable<sql_types::Integer>)]
    pub width: Option<i32>,
    /// Video height in pixels.
    #[diesel(sql_type = sql_types::Nullable<sql_types::Integer>)]
    pub height: Option<i32>,
    /// Probed video codec.
    #[diesel(sql_type = sql_types::Nullable<sql_types::Text>)]
    pub video_codec: Option<String>,
    /// Probed audio codec.
    #[diesel(sql_type = sql_types::Nullable<sql_types::Text>)]
    pub audio_codec: Option<String>,
}

/// Paginated source files and totals for a catalog subtree.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MediaItemSources {
    /// Requested catalog item.
    pub item_id: i32,
    /// Local title before metadata overrides.
    pub catalog_title: String,
    /// Effective zero-based offset.
    pub offset: i64,
    /// Effective page size, at most 100.
    pub limit: i64,
    /// Number of backing files, including missing files.
    pub total_files: i64,
    /// Number of missing backing files.
    pub missing_files: i64,
    /// Combined size of all backing files in bytes.
    pub total_size: i64,
    /// Files on the requested page.
    pub files: Vec<MediaItemSourceFile>,
}

// UNION deduplicates identifiers even if a corrupt hierarchy contains a cycle.
// Both edges and memberships stay within the requested item's library.
const SOURCES: &str = "WITH RECURSIVE descendants(id, library_id) AS (
    SELECT id, library_id FROM media_items WHERE id = ? AND deleted_at IS NULL
    UNION
    SELECT child.id, child.library_id FROM media_items child
    JOIN descendants parent ON child.parent_id = parent.id
        AND child.library_id = parent.library_id
    WHERE child.deleted_at IS NULL
)
";
const SOURCE_JOINS: &str = "FROM descendants d
    JOIN media_items item ON item.id = d.id
    JOIN media_file_libraries membership ON membership.media_item_id = d.id
        AND membership.library_id = d.library_id AND membership.deleted_at IS NULL
    JOIN media_files file ON file.id = membership.media_file_id";

#[derive(diesel::QueryableByName)]
struct Totals {
    #[diesel(sql_type = sql_types::BigInt)]
    total_files: i64,
    #[diesel(sql_type = sql_types::BigInt)]
    missing_files: i64,
    #[diesel(sql_type = sql_types::BigInt)]
    total_size: i64,
}

/// Inspect an active item without loading provider payloads or assuming an item type.
pub fn get_item_source_files(
    conn: &mut SqliteConnection,
    item_id: i32,
    offset: i64,
    limit: i64,
) -> Result<Option<MediaItemSources>, diesel::result::Error> {
    use crate::db::schema::media_items::dsl as items;
    let Some(catalog_title) = items::media_items
        .find(item_id)
        .filter(items::deleted_at.is_null())
        .select(items::display_title)
        .first::<String>(conn)
        .optional()?
    else {
        return Ok(None);
    };
    let offset = offset.max(0);
    let limit = limit.clamp(1, 100);
    let totals = diesel::sql_query(format!(
        "{SOURCES} SELECT COUNT(*) AS total_files,
        COALESCE(SUM(membership.missing_since IS NOT NULL), 0) AS missing_files,
        COALESCE(SUM(file.file_size), 0) AS total_size {SOURCE_JOINS}"
    ))
    .bind::<sql_types::Integer, _>(item_id)
    .get_result::<Totals>(conn)?;
    let files = diesel::sql_query(format!(
        "{SOURCES} SELECT membership.id, item.id AS item_id, item.item_type,
        item.display_title, file.path, membership.source_root_path, membership.relative_path,
        file.file_size, file.modified_at, membership.missing_since, file.container,
        file.duration_ms, file.bit_rate, file.width, file.height, file.video_codec,
        file.audio_codec {SOURCE_JOINS}
        ORDER BY membership.source_root_path COLLATE NOCASE,
        membership.relative_path COLLATE NOCASE, membership.id LIMIT ? OFFSET ?"
    ))
    .bind::<sql_types::Integer, _>(item_id)
    .bind::<sql_types::BigInt, _>(limit)
    .bind::<sql_types::BigInt, _>(offset)
    .load::<MediaItemSourceFile>(conn)?;
    Ok(Some(MediaItemSources {
        item_id,
        catalog_title,
        offset,
        limit,
        total_files: totals.total_files,
        missing_files: totals.missing_files,
        total_size: totals.total_size,
        files,
    }))
}
