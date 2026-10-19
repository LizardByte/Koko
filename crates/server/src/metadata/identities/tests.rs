use diesel::{
    Connection,
    connection::SimpleConnection,
};

use super::*;

#[test]
fn links_registry_ids_transitively_across_providers_and_locales_without_names_or_social_accounts() {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    conn.batch_execute(
        "
        INSERT INTO media_libraries (id,name,path,kind) VALUES (1,'Music','music','music');
        INSERT INTO media_items (id,library_id,identity_key,item_type,display_title,deleted_at) \
         VALUES
            (10,1,'a','artist','Artist',NULL), (11,1,'b','artist','Artist',NULL),
            (12,1,'c','album','Artist',NULL), (13,1,'d','artist','Artist',NULL),
            (14,1,'e','artist','Artist',123), (15,1,'f','artist','Artist',NULL);
        INSERT INTO item_metadata_links \
         (id,media_item_id,provider_id,external_id,media_type,relation_kind,match_state) VALUES
            (10,10,'musicbrainz','artist-a','artist','primary','linked'),
            (11,11,'musicbrainz','artist-a','artist','primary','linked'),
            (12,12,'musicbrainz','album','release','primary','linked'),
            (13,13,'musicbrainz','artist-a','artist','primary','unlinked'),
            (14,14,'musicbrainz','artist-a','artist','primary','linked'),
            (15,15,'musicbrainz','artist-a','artist','secondary','linked');
        INSERT INTO item_metadata_external_ids (metadata_link_id,source,external_id) VALUES
            (10,'imdb','nm1'), (12,'imdb','nm1');
        INSERT INTO metadata_people (id,provider_id,external_id,locale_key,name) VALUES
            (1,'tmdb','123','en-US','Artist'), (2,'tmdb','123','fr-FR','Artiste'),
            (3,'tvdb','456','en-US','Different credit name'),
            (4,'tmdb','789','en-US','Artist'), (5,'tmdb','101','en-US','Band member');
        INSERT INTO metadata_person_external_ids (person_id,source,external_id) VALUES
            (1,'imdb','nm1'), (1,'wikidata','Q1'), (1,'facebook','shared-account'),
            (3,'wikidata','Q1'), (4,'imdb','nm2'), (5,'facebook','shared-account');
    ",
    )
    .unwrap();

    for identities in [
        for_artist(&mut conn, 10).unwrap(),
        for_person(&mut conn, 2).unwrap(),
        for_person(&mut conn, 3).unwrap(),
    ] {
        assert_eq!(identities.person_ids, [1, 2, 3]);
        assert_eq!(identities.artist_ids, [10, 11]);
    }
    let unrelated = for_person(&mut conn, 4).unwrap();
    assert_eq!(unrelated.person_ids, [4]);
    assert!(unrelated.artist_ids.is_empty());

    // Explicit unlinking immediately disconnects the artist's former identifiers.
    conn.batch_execute("UPDATE item_metadata_links SET match_state='unlinked' WHERE id=10")
        .unwrap();
    let unlinked = for_artist(&mut conn, 10).unwrap();
    assert!(unlinked.person_ids.is_empty());
    assert_eq!(
        for_person(&mut conn, 1).unwrap().artist_ids,
        Vec::<i32>::new()
    );
}
