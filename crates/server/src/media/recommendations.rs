//! Cached same-type recommendations, maintained during metadata changes and library scans.

mod cache;
pub(crate) use cache::refresh;

use super::*;
use crate::db::schema::{
    item_metadata_external_ids as external,
    item_metadata_links as links,
    item_metadata_people as people,
    item_recommendations as cached,
    item_similarities as similar,
    media_items as item_rows,
    metadata_collection_items as members,
};
use crate::metadata::similarities::Identity;
use diesel::{
    JoinOnDsl,
    QueryResult,
};

const LIMIT: usize = 12;
const EXCLUDED_ITEM_TYPES: [&str; 2] = ["season", "episode"];

#[derive(Clone, Default)]
struct Facts {
    genres: HashSet<String>,
    creators: HashSet<String>,
    collections: HashSet<String>,
    identities: HashSet<Identity>,
    artists: HashSet<Identity>,
}

fn normalized(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn add_text(
    set: &mut HashSet<String>,
    value: &str,
) {
    let value = normalized(value);
    if !value.is_empty() && !value.starts_with("unknown ") {
        set.insert(value);
    }
}

fn lineage(
    id: i32,
    items: &HashMap<i32, MediaItemSummary>,
) -> Vec<i32> {
    let mut ids = Vec::new();
    let mut current = Some(id);
    while let Some(id) = current {
        if ids.contains(&id) {
            break;
        }
        ids.push(id);
        current = items.get(&id).and_then(|item| item.parent_id);
    }
    ids
}

fn local_facts(item: &MediaItemSummary) -> Facts {
    let mut facts = Facts::default();
    for genre in &item.genres {
        add_text(&mut facts.genres, genre);
    }
    if ["artist", "book_author"].contains(&item.item_type.as_str()) {
        add_text(&mut facts.creators, &item.display_title);
    }
    if let Some(music) = &item.music {
        add_text(&mut facts.creators, &music.artist);
        add_text(&mut facts.creators, &music.album_artist);
        for genre in &music.genres {
            add_text(&mut facts.genres, genre);
        }
        if let Some(id) = music
            .musicbrainz_artist_id
            .as_deref()
            .filter(|id| valid_mbid(id))
        {
            facts
                .artists
                .insert(Identity::new("musicbrainz", "artist", id));
        }
    }
    if let Some(book) = &item.audiobook {
        add_text(&mut facts.creators, &book.author);
        if let Some(series) = &book.series {
            add_text(&mut facts.collections, series);
        }
    }
    facts
}

fn own_kind(item_type: &str) -> &str {
    match item_type {
        "show" => "tv",
        "album" => "release",
        "track" => "recording",
        "book_author" => "author",
        "book_work" | "book" => "book",
        other => other,
    }
}

fn load_facts(
    conn: &mut SqliteConnection,
    items: &HashMap<i32, MediaItemSummary>,
) -> QueryResult<HashMap<i32, Facts>> {
    let mut facts: HashMap<_, _> = items
        .iter()
        .map(|(id, item)| (*id, local_facts(item)))
        .collect();
    let ids: Vec<_> = items.keys().copied().collect();
    for chunk in ids.chunks(500) {
        let active = || {
            links::table
                .filter(links::media_item_id.eq_any(chunk))
                .filter(links::relation_kind.eq("primary"))
                .filter(links::match_state.eq("linked"))
        };
        for (id, provider, kind, external_id) in active()
            .select((
                links::media_item_id,
                links::provider_id,
                links::media_type,
                links::external_id,
            ))
            .load::<(i32, String, Option<String>, String)>(conn)?
        {
            let item_facts = facts.get_mut(&id).expect("authorized item");
            let kind = kind.unwrap_or_else(|| own_kind(&items[&id].item_type).into());
            let identity = Identity::new(&provider, &kind, &external_id);
            if provider == "musicbrainz" && kind == "artist" {
                item_facts.artists.insert(identity.clone());
            }
            item_facts.identities.insert(identity);
        }
        for (id, source, value) in external::table
            .inner_join(links::table)
            .filter(links::media_item_id.eq_any(chunk))
            .filter(links::relation_kind.eq("primary"))
            .filter(links::match_state.eq("linked"))
            .select((
                links::media_item_id,
                external::source,
                external::external_id,
            ))
            .load::<(i32, String, String)>(conn)?
        {
            let facts = facts.get_mut(&id).expect("authorized item");
            match source.as_str() {
                "tmdb" if ["movie", "show"].contains(&items[&id].item_type.as_str()) => {
                    facts.identities.insert(Identity::new(
                        "tmdb",
                        own_kind(&items[&id].item_type),
                        &value,
                    ));
                }
                "musicbrainz_artist" => {
                    facts
                        .artists
                        .insert(Identity::new("musicbrainz", "artist", &value));
                }
                "musicbrainz_release_group" => {
                    facts
                        .identities
                        .insert(Identity::new("musicbrainz", "release-group", &value));
                }
                _ => {}
            }
        }
        for (id, name, role) in people::table
            .inner_join(links::table)
            .filter(links::media_item_id.eq_any(chunk))
            .filter(links::relation_kind.eq("primary"))
            .filter(links::match_state.eq("linked"))
            .select((links::media_item_id, people::name, people::role))
            .load::<(i32, String, Option<String>)>(conn)?
        {
            if role.as_deref().is_some_and(|role| {
                [
                    "author", "writer", "director", "creator", "composer",
                ]
                .iter()
                .any(|kind| role.to_lowercase().contains(kind))
            }) {
                add_text(
                    &mut facts.get_mut(&id).expect("authorized item").creators,
                    &name,
                );
            }
        }
        for (id, collection) in members::table
            .filter(members::media_item_id.eq_any(chunk))
            .select((members::media_item_id, members::collection_id))
            .load::<(i32, i32)>(conn)?
        {
            facts
                .get_mut(&id)
                .expect("authorized item")
                .collections
                .insert(format!("collection:{collection}"));
        }
    }
    // Container genres can come from their tracks or works, even without a separate artist/author
    // genre.
    for (id, own) in facts.clone() {
        for ancestor in lineage(id, items).into_iter().skip(1) {
            if let Some(parent) = facts.get_mut(&ancestor) {
                parent.genres.extend(own.genres.iter().cloned());
            }
        }
    }
    Ok(facts)
}

fn inherited(
    id: i32,
    items: &HashMap<i32, MediaItemSummary>,
    facts: &HashMap<i32, Facts>,
) -> Facts {
    let mut result = Facts::default();
    for id in lineage(id, items) {
        if let Some(facts) = facts.get(&id) {
            if result.genres.is_empty() {
                result.genres.extend(facts.genres.iter().cloned());
            }
            result.creators.extend(facts.creators.iter().cloned());
            result.collections.extend(facts.collections.iter().cloned());
            result.identities.extend(facts.identities.iter().cloned());
            result.artists.extend(facts.artists.iter().cloned());
        }
    }
    result
}

/// Read cached matches, filtering permissions and availability before the result limit.
/// Neither fallback scoring nor provider discovery runs on item-page requests.
pub(crate) fn list(
    conn: &mut SqliteConnection,
    user_id: Option<i32>,
    item_id: i32,
    languages: &[String],
) -> QueryResult<Vec<MediaItemSummary>> {
    let seed = load_media_item_row(conn, item_id)?.ok_or(diesel::result::Error::NotFound)?;
    let visible = accessible_library_ids(conn, user_id)?;
    if !visible.contains(&seed.library_id) {
        return Err(diesel::result::Error::NotFound);
    }
    if EXCLUDED_ITEM_TYPES.contains(&seed.item_type.as_str()) {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let mut offset = 0;
    // Fetch small batches so inaccessible or duplicate candidates cannot consume the visible limit.
    while result.len() < LIMIT {
        let rows = cached::table
            .inner_join(item_rows::table.on(item_rows::id.eq(cached::similar_item_id)))
            .filter(cached::item_id.eq(item_id))
            .filter(item_rows::id.ne(item_id))
            .filter(item_rows::item_type.eq(&seed.item_type))
            .filter(item_rows::library_id.eq_any(&visible))
            .filter(item_rows::missing_since.is_null())
            .filter(item_rows::deleted_at.is_null())
            .order((
                cached::provider_priority.desc(),
                cached::provider_rank.asc(),
                cached::local_score.desc(),
                cached::year_distance.asc(),
                item_rows::display_title.asc(),
                item_rows::id.asc(),
            ))
            .offset(offset)
            .limit(48)
            .select(MediaItem::as_select())
            .load::<MediaItem>(conn)?;
        if rows.is_empty() {
            break;
        }
        offset += rows.len() as i64;
        let identities = row_identities(conn, &rows)?;
        for row in rows {
            let keys = &identities[&row.id];
            if !keys.is_disjoint(&seen) {
                continue;
            }
            seen.extend(keys.iter().cloned());
            result.push(media_item_summary_with_preferred_languages(
                conn, row, languages,
            )?);
            if result.len() == LIMIT {
                break;
            }
        }
    }
    apply_user_playback_progress_to_summaries(conn, user_id, &mut result)?;
    Ok(result)
}

/// Read only identity keys for one bounded batch, without loading catalog facts or scoring it.
fn row_identities(
    conn: &mut SqliteConnection,
    rows: &[MediaItem],
) -> QueryResult<HashMap<i32, HashSet<Identity>>> {
    let kinds: HashMap<_, _> = rows
        .iter()
        .map(|row| (row.id, own_kind(&row.item_type)))
        .collect();
    let ids: Vec<_> = kinds.keys().copied().collect();
    let mut result: HashMap<_, HashSet<_>> = ids.iter().map(|id| (*id, HashSet::new())).collect();
    for (id, provider, kind, external_id) in links::table
        .filter(links::media_item_id.eq_any(&ids))
        .filter(links::relation_kind.eq("primary"))
        .filter(links::match_state.eq("linked"))
        .select((
            links::media_item_id,
            links::provider_id,
            links::media_type,
            links::external_id,
        ))
        .load::<(i32, String, Option<String>, String)>(conn)?
    {
        result
            .get_mut(&id)
            .expect("candidate")
            .insert(Identity::new(
                &provider,
                kind.as_deref().unwrap_or(kinds[&id]),
                &external_id,
            ));
    }
    for (id, source, external_id) in external::table
        .inner_join(links::table)
        .filter(links::media_item_id.eq_any(&ids))
        .filter(links::relation_kind.eq("primary"))
        .filter(links::match_state.eq("linked"))
        .select((
            links::media_item_id,
            external::source,
            external::external_id,
        ))
        .load::<(i32, String, String)>(conn)?
    {
        let identity = match source.as_str() {
            "tmdb" if ["movie", "tv"].contains(&kinds[&id]) => {
                Some(Identity::new("tmdb", kinds[&id], &external_id))
            }
            "musicbrainz_release_group" => {
                Some(Identity::new("musicbrainz", "release-group", &external_id))
            }
            _ => None,
        };
        if let Some(identity) = identity {
            result.get_mut(&id).expect("candidate").insert(identity);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
