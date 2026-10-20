use diesel::{
    Connection,
    SqliteConnection,
    connection::SimpleConnection,
};
use koko::media::playlists::{
    self,
    QueueState,
};

fn database() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    koko::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    koko::db::configure_sqlite_connection(&mut conn).unwrap();
    conn.batch_execute(
        "INSERT INTO users(id,username,password,admin) VALUES
         (1,'admin','hash',1),(2,'viewer','hash',0);
         INSERT INTO media_libraries(id,name,path,kind,allowed_user_ids_json) VALUES
         (1,'Seed','/seed','music','[]'),(2,'Private','/private','music','[3]'),
         (3,'Other','/other','music','[]');
         INSERT INTO media_items(id,library_id,parent_id,identity_key,item_type,display_title,
                                 playable,media_kind) VALUES
         (10,1,NULL,'artist','artist','Seed Artist',0,'audio'),
         (11,1,10,'album','album','Seed Album',0,'audio'),
         (12,1,11,'song','track','Seed Song',1,'audio'),
         (20,3,NULL,'other-artist','artist','Other Artist',0,'audio'),
         (21,3,20,'other-album','album','Other Album',0,'audio'),
         (22,3,21,'other-song','track','Other Song',1,'audio'),
         (30,2,NULL,'private-artist','artist','Private Artist',0,'audio');
         INSERT INTO item_metadata_links(media_item_id,provider_id,external_id,title,updated_at)
         VALUES(10,'musicbrainz','artist','Seed Artist',100),
               (11,'musicbrainz','album','Seed Album',100);",
    )
    .unwrap();
    conn
}

#[test]
fn radio_artwork_tracks_the_original_artist_or_album_through_queue_edits_and_artwork_refreshes() {
    let mut conn = database();
    for (seed, expected) in [(10, 10), (11, 11), (12, 11)] {
        let saved =
            playlists::create_radio(&mut conn, 2, "Radio", &[12, 22, 12], Some(seed)).unwrap();
        assert_eq!(saved.playlist.artwork_item_id, Some(expected));
        assert_eq!(saved.artwork.as_ref().unwrap().item_id, expected);
        playlists::update_queue(
            &mut conn,
            2,
            saved.playlist.id,
            &QueueState {
                item_ids: vec![22, 12],
                current_index: 1,
                position_ms: 123000,
                completed: false,
                shuffle: true,
                repeat_mode: "all".into(),
            },
        )
        .unwrap();
        let changed = playlists::load(&mut conn, 2, saved.playlist.id).unwrap();
        assert_eq!(changed.artwork, saved.artwork);
        assert_eq!(changed.items[0].id, 22);
        conn.batch_execute(&format!(
            "UPDATE item_metadata_links SET updated_at={} WHERE media_item_id={expected};",
            1000 + seed
        ))
        .unwrap();
        let refreshed = playlists::load(&mut conn, 2, saved.playlist.id).unwrap();
        assert_eq!(
            refreshed.artwork.unwrap().updated_at,
            Some((1000 + seed).into())
        );
        let promoted =
            playlists::rename_and_save(&mut conn, 2, saved.playlist.id, "Saved radio").unwrap();
        assert_eq!(promoted.playlist.artwork_item_id, Some(expected));
        assert!(!promoted.playlist.temporary);
    }
    conn.batch_execute("DELETE FROM media_items WHERE id=11;")
        .unwrap();
    let saved = playlists::list(&mut conn, 2, None).unwrap();
    assert_eq!(
        saved.iter().filter(|queue| queue.artwork.is_none()).count(),
        2
    );
    assert_eq!(
        saved
            .iter()
            .filter_map(|queue| queue.artwork.as_ref())
            .map(|artwork| artwork.item_id)
            .collect::<Vec<_>>(),
        vec![10]
    );
}

#[test]
fn artwork_requires_seed_access_and_does_not_leak_after_access_is_revoked() {
    let mut conn = database();
    assert!(matches!(
        playlists::create_radio(&mut conn, 2, "Private", &[22], Some(30)),
        Err(diesel::result::Error::NotFound)
    ));
    assert!(playlists::list(&mut conn, 2, None).unwrap().is_empty());
    let saved = playlists::create_radio(&mut conn, 2, "Visible", &[22], Some(10)).unwrap();
    assert!(saved.artwork.is_some());
    conn.batch_execute("UPDATE media_libraries SET allowed_user_ids_json='[3]' WHERE id=1;")
        .unwrap();
    let revoked = playlists::load(&mut conn, 2, saved.playlist.id).unwrap();
    assert!(revoked.artwork.is_none());
    assert_eq!(revoked.playlist.artwork_item_id, None);
    assert_eq!(revoked.items[0].id, 22);
    assert!(matches!(
        playlists::load(&mut conn, 1, saved.playlist.id),
        Err(diesel::result::Error::NotFound)
    ));
}

#[test]
fn combined_music_migration_applies_and_reverts_as_one_revision() {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    let revisions = koko::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    assert_eq!(
        revisions
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["a54d52c8da5e", "c18f43d2a9b7"]
    );
    assert!(
        koko::db::run_pending_sqlite_migrations(&mut conn)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        koko::db::revert_all_sqlite_migrations(&mut conn)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        koko::db::run_pending_sqlite_migrations(&mut conn)
            .unwrap()
            .len(),
        2
    );
}
