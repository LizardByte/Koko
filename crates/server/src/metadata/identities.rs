//! Connect provider people and library artists through exact registry IDs.

#[cfg(test)]
mod tests;

use std::collections::{
    BTreeSet,
    HashSet,
    VecDeque,
};

use crate::db::schema::{
    item_metadata_external_ids as external_ids,
    item_metadata_links as links,
    media_items as items,
    metadata_people as persons,
    metadata_person_external_ids as person_ids,
};
use diesel::{
    SqliteConnection,
    prelude::*,
};

/// Equivalent provider people and library artists, resolved through registry IDs.
#[derive(Default, Debug)]
pub struct LinkedIdentities {
    /// Person rows, including localized provider records.
    pub person_ids: Vec<i32>,
    /// Live, linked artist items across libraries.
    pub artist_ids: Vec<i32>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Node {
    Person(i32),
    Artist(i32),
}

/// Resolve identities connected to an existing provider person.
pub fn for_person(
    conn: &mut SqliteConnection,
    id: i32,
) -> Result<LinkedIdentities, diesel::result::Error> {
    // Preserve the existing fallback for localized people without provider IDs.
    let seeds = super::get_metadata_person_locale_peer_ids(conn, id)?
        .into_iter()
        .map(Node::Person);
    resolve(conn, seeds)
}

/// Resolve identities connected to a library artist.
pub fn for_artist(
    conn: &mut SqliteConnection,
    id: i32,
) -> Result<LinkedIdentities, diesel::result::Error> {
    resolve(conn, [Node::Artist(id)])
}

fn resolve(
    conn: &mut SqliteConnection,
    seeds: impl IntoIterator<Item = Node>,
) -> Result<LinkedIdentities, diesel::result::Error> {
    let mut queue = seeds.into_iter().collect::<VecDeque<_>>();
    let mut visited = HashSet::new();
    let mut identifiers = HashSet::new();
    let mut people = BTreeSet::new();
    let mut artists = BTreeSet::new();
    while let Some(node) = queue.pop_front() {
        if !visited.insert(node) {
            continue;
        }
        let ids = match node {
            Node::Person(id) => {
                people.insert(id);
                person_identifiers(conn, id)?
            }
            Node::Artist(id) => {
                artists.insert(id);
                artist_identifiers(conn, id)?
            }
        };
        for (id_source, external_id) in ids {
            let Some(source) = super::normalize_external_id_source(&id_source) else { continue };
            // Social profiles and homepages can be shared by bands and members.
            // Entity registry IDs identify the person rather than their account.
            if !matches!(
                source.as_str(),
                "imdb"
                    | "wikidata"
                    | "tmdb"
                    | "thetvdb"
                    | "musicbrainz"
                    | "discogs"
                    | "isni"
                    | "viaf"
            ) || external_id.is_empty()
                || !identifiers.insert((source.clone(), external_id.clone()))
            {
                continue;
            }
            let provider = if source == "thetvdb" { "tvdb" } else { &source };
            queue.extend(
                matching_people(conn, &source, provider, &external_id)?
                    .into_iter()
                    .map(Node::Person),
            );
            queue.extend(
                matching_artists(conn, &source, provider, &external_id)?
                    .into_iter()
                    .map(Node::Artist),
            );
        }
    }
    Ok(LinkedIdentities {
        person_ids: people.into_iter().collect(),
        artist_ids: artists.into_iter().collect(),
    })
}

fn person_identifiers(
    conn: &mut SqliteConnection,
    id: i32,
) -> QueryResult<Vec<(String, String)>> {
    let mut ids = person_ids::table
        .filter(person_ids::person_id.eq(id))
        .select((person_ids::source, person_ids::external_id))
        .load(conn)?;
    if let Some((provider, Some(external))) = persons::table
        .find(id)
        .select((persons::provider_id, persons::external_id))
        .first::<(String, Option<String>)>(conn)
        .optional()?
    {
        ids.push((provider, external));
    }
    Ok(ids)
}

fn artist_identifiers(
    conn: &mut SqliteConnection,
    id: i32,
) -> QueryResult<Vec<(String, String)>> {
    let mut ids = external_ids::table
        .inner_join(links::table)
        .filter(links::media_item_id.eq(id))
        .filter(links::relation_kind.eq("primary"))
        .filter(links::match_state.ne("unlinked"))
        .filter(links::media_type.eq("artist"))
        .select((external_ids::source, external_ids::external_id))
        .load(conn)?;
    ids.extend(
        links::table
            .filter(links::media_item_id.eq(id))
            .filter(links::relation_kind.eq("primary"))
            .filter(links::match_state.ne("unlinked"))
            .filter(links::media_type.eq("artist"))
            .select((links::provider_id, links::external_id))
            .load::<(String, String)>(conn)?,
    );
    Ok(ids)
}

fn matching_people(
    conn: &mut SqliteConnection,
    source: &str,
    provider: &str,
    external: &str,
) -> QueryResult<Vec<i32>> {
    let mut ids = person_ids::table
        .filter(person_ids::source.eq(source))
        .filter(person_ids::external_id.eq(external))
        .select(person_ids::person_id)
        .load::<i32>(conn)?;
    ids.extend(
        persons::table
            .filter(persons::provider_id.eq(provider))
            .filter(persons::external_id.eq(external))
            .select(persons::id)
            .load::<i32>(conn)?,
    );
    Ok(ids)
}

fn matching_artists(
    conn: &mut SqliteConnection,
    source: &str,
    provider: &str,
    external: &str,
) -> QueryResult<Vec<i32>> {
    let mut ids = external_ids::table
        .inner_join(links::table.inner_join(items::table))
        .filter(external_ids::source.eq(source))
        .filter(external_ids::external_id.eq(external))
        .filter(links::relation_kind.eq("primary"))
        .filter(links::match_state.ne("unlinked"))
        .filter(links::media_type.eq("artist"))
        .filter(items::item_type.eq("artist"))
        .filter(items::deleted_at.is_null())
        .select(links::media_item_id)
        .load::<i32>(conn)?;
    ids.extend(
        links::table
            .inner_join(items::table)
            .filter(links::provider_id.eq(provider))
            .filter(links::external_id.eq(external))
            .filter(links::relation_kind.eq("primary"))
            .filter(links::match_state.ne("unlinked"))
            .filter(links::media_type.eq("artist"))
            .filter(items::item_type.eq("artist"))
            .filter(items::deleted_at.is_null())
            .select(links::media_item_id)
            .load::<i32>(conn)?,
    );
    Ok(ids)
}
