//! Connect provider people and library artists through exact registry IDs.

#[cfg(test)]
mod tests;

use std::collections::{
    BTreeSet,
    HashSet,
    VecDeque,
};

use diesel::{
    QueryableByName,
    RunQueryDsl,
    SqliteConnection,
    sql_types::{
        Integer,
        Text,
    },
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

#[derive(QueryableByName)]
struct Identifier {
    #[diesel(sql_type = Text)]
    source: String,
    #[diesel(sql_type = Text)]
    external_id: String,
}

#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type = Integer)]
    id: i32,
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
                diesel::sql_query(
                    "SELECT source, external_id FROM metadata_person_external_ids WHERE person_id \
                     = ? UNION SELECT provider_id AS source, external_id FROM metadata_people \
                     WHERE id = ? AND external_id IS NOT NULL",
                )
                .bind::<Integer, _>(id)
                .bind::<Integer, _>(id)
                .load::<Identifier>(conn)?
            }
            Node::Artist(id) => {
                artists.insert(id);
                diesel::sql_query(
                    "SELECT e.source, e.external_id FROM item_metadata_external_ids e JOIN \
                     item_metadata_links l ON l.id=e.metadata_link_id WHERE l.media_item_id=? AND \
                     l.relation_kind='primary' AND l.match_state<>'unlinked' AND \
                     l.media_type='artist' UNION SELECT provider_id AS source, external_id FROM \
                     item_metadata_links WHERE media_item_id=? AND relation_kind='primary' AND \
                     match_state<>'unlinked' AND media_type='artist'",
                )
                .bind::<Integer, _>(id)
                .bind::<Integer, _>(id)
                .load::<Identifier>(conn)?
            }
        };
        for id in ids {
            let Some(source) = super::normalize_external_id_source(&id.source) else { continue };
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
            ) || id.external_id.is_empty()
                || !identifiers.insert((source.clone(), id.external_id.clone()))
            {
                continue;
            }
            let provider = if source == "thetvdb" { "tvdb" } else { &source };
            let people = diesel::sql_query(
                "SELECT person_id AS id FROM metadata_person_external_ids WHERE source=? AND \
                 external_id=? UNION SELECT id FROM metadata_people WHERE provider_id=? AND \
                 external_id=?",
            )
            .bind::<Text, _>(&source)
            .bind::<Text, _>(&id.external_id)
            .bind::<Text, _>(provider)
            .bind::<Text, _>(&id.external_id)
            .load::<Id>(conn)?;
            queue.extend(people.into_iter().map(|row| Node::Person(row.id)));
            let artists = diesel::sql_query(
                "SELECT l.media_item_id AS id FROM item_metadata_external_ids e JOIN \
                 item_metadata_links l ON l.id=e.metadata_link_id JOIN media_items m ON \
                 m.id=l.media_item_id WHERE e.source=? AND e.external_id=? AND \
                 l.relation_kind='primary' AND l.match_state<>'unlinked' AND \
                 l.media_type='artist' AND m.item_type='artist' AND m.deleted_at IS NULL UNION \
                 SELECT l.media_item_id AS id FROM item_metadata_links l JOIN media_items m ON \
                 m.id=l.media_item_id WHERE l.provider_id=? AND l.external_id=? AND \
                 l.relation_kind='primary' AND l.match_state<>'unlinked' AND \
                 l.media_type='artist' AND m.item_type='artist' AND m.deleted_at IS NULL",
            )
            .bind::<Text, _>(&source)
            .bind::<Text, _>(&id.external_id)
            .bind::<Text, _>(provider)
            .bind::<Text, _>(&id.external_id)
            .load::<Id>(conn)?;
            queue.extend(artists.into_iter().map(|row| Node::Artist(row.id)));
        }
    }
    Ok(LinkedIdentities {
        person_ids: people.into_iter().collect(),
        artist_ids: artists.into_iter().collect(),
    })
}
