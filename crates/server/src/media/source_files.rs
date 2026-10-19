//! Bounded, scalar-only source inspection shared by every catalog item type.

#[cfg(test)]
mod tests;

use diesel::sql_types::{
    BigInt,
    Nullable,
    Text,
};
use diesel::{
    ExpressionMethods,
    OptionalExtension,
    QueryDsl,
    RunQueryDsl,
    SqliteConnection,
    prelude::*,
};
use rocket_okapi::okapi::schemars::{
    self,
    JsonSchema,
};
use serde::Serialize;

/// One physical file belonging to an item or any of its descendants.
#[derive(Debug, Serialize, JsonSchema, diesel::Queryable)]
pub struct MediaItemSourceFile {
    /// Stable library file membership identifier.
    pub id: i32,
    /// Catalog item backed by this file.
    pub item_id: i32,
    /// Catalog item type, including future types.
    pub item_type: String,
    /// Local scanner title before provider presentation overrides.
    pub display_title: String,
    /// Full physical path on the server.
    pub path: String,
    /// Library root that discovered the file.
    pub source_root_path: String,
    /// Path relative to the library root.
    pub relative_path: String,
    /// Physical file size in bytes.
    pub file_size: i64,
    /// File modification time in Unix seconds.
    pub modified_at: Option<i64>,
    /// Time the scanner first noticed this file was missing.
    pub missing_since: Option<i64>,
    /// Probed container name.
    pub container: Option<String>,
    /// Probed duration in milliseconds.
    pub duration_ms: Option<i64>,
    /// Probed bitrate in bits per second.
    pub bit_rate: Option<i64>,
    /// Video width in pixels.
    pub width: Option<i32>,
    /// Video height in pixels.
    pub height: Option<i32>,
    /// Probed video codec.
    pub video_codec: Option<String>,
    /// Probed audio codec.
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

diesel::define_sql_function! {
    fn lower(value: Text) -> Text;
}
diesel::define_sql_function! {
    #[aggregate]
    #[sql_name = "SUM"]
    fn sum_bytes(value: BigInt) -> Nullable<BigInt>;
}

/// Inspect an active item without loading provider payloads or assuming an item type.
pub fn get_item_source_files(
    conn: &mut SqliteConnection,
    item_id: i32,
    offset: i64,
    limit: i64,
) -> QueryResult<Option<MediaItemSources>> {
    use crate::db::schema::{
        media_file_libraries as membership,
        media_files as file,
        media_items as items,
    };
    let Some((catalog_title, library)) = items::table
        .find(item_id)
        .filter(items::deleted_at.is_null())
        .select((items::display_title, items::library_id))
        .first::<(String, i32)>(conn)
        .optional()?
    else {
        return Ok(None);
    };
    let offset = offset.max(0);
    let limit = limit.clamp(1, 100);
    let ids = descendant_ids(conn, item_id, library)?;
    let mut total_files = 0;
    let mut missing_files = 0;
    let mut total_size = 0;
    let mut files = Vec::new();
    for chunk in ids.chunks(500) {
        let sources = || {
            membership::table
                .inner_join(items::table)
                .inner_join(file::table)
                .filter(membership::media_item_id.eq_any(chunk))
                .filter(membership::library_id.eq(library))
                .filter(membership::deleted_at.is_null())
        };
        let (count, size) = sources()
            .select((diesel::dsl::count_star(), sum_bytes(file::file_size)))
            .first::<(i64, Option<i64>)>(conn)?;
        total_files += count;
        total_size += size.unwrap_or_default();
        missing_files += sources()
            .filter(membership::missing_since.is_not_null())
            .count()
            .get_result::<i64>(conn)?;
        files.extend(
            sources()
                .order((
                    lower(membership::source_root_path),
                    lower(membership::relative_path),
                    membership::id,
                ))
                .limit(offset.saturating_add(limit))
                .select((
                    membership::id,
                    items::id,
                    items::item_type,
                    items::display_title,
                    file::path,
                    membership::source_root_path,
                    membership::relative_path,
                    file::file_size,
                    file::modified_at,
                    membership::missing_since,
                    file::container,
                    file::duration_ms,
                    file::bit_rate,
                    file::width,
                    file::height,
                    file::video_codec,
                    file::audio_codec,
                ))
                .load::<MediaItemSourceFile>(conn)?,
        );
    }
    files.sort_by_cached_key(|file| {
        (
            file.source_root_path.to_ascii_lowercase(),
            file.relative_path.to_ascii_lowercase(),
            file.id,
        )
    });
    let files = files
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();
    Ok(Some(MediaItemSources {
        item_id,
        catalog_title,
        offset,
        limit,
        total_files,
        missing_files,
        total_size,
        files,
    }))
}

fn descendant_ids(
    conn: &mut SqliteConnection,
    root: i32,
    library: i32,
) -> QueryResult<Vec<i32>> {
    use crate::db::schema::media_items as items;
    let mut visited = std::collections::HashSet::from([root]);
    let mut pending = vec![root];
    while !pending.is_empty() {
        let mut next = Vec::new();
        for chunk in pending.chunks(500) {
            for id in items::table
                .filter(items::parent_id.eq_any(chunk))
                .filter(items::library_id.eq(library))
                .filter(items::deleted_at.is_null())
                .select(items::id)
                .load::<i32>(conn)?
            {
                if visited.insert(id) {
                    next.push(id);
                }
            }
        }
        pending = next;
    }
    Ok(visited.into_iter().collect())
}
