use diesel::{
    Connection,
    SqliteConnection,
    connection::SimpleConnection,
};
use koko::{
    config::MediaLibrarySettings,
    media,
};

fn database() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    koko::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    koko::db::configure_sqlite_connection(&mut conn).unwrap();
    conn.batch_execute("INSERT INTO users(id,username,password,admin) VALUES(1,'admin','hash',1);")
        .unwrap();
    conn
}

#[test]
fn exclusion_round_trips_for_every_library_kind_and_only_filters_combined_home() {
    let mut conn = database();
    for (index, kind) in [
        "movies",
        "shows",
        "music",
        "books",
        "photos",
        "home_videos",
        "mixed",
    ]
    .iter()
    .enumerate()
    {
        let id = index + 1;
        conn.batch_execute(&format!(
            "INSERT INTO media_libraries(id,name,path,kind) VALUES({id},'Library \
             {id}','/{id}','{kind}');
            INSERT INTO media_items(id,library_id,identity_key,item_type,display_title,playable)
            VALUES({id},{id},'item-{id}','file','Item {id}',1);
            INSERT INTO \
             playback_progress(user_id,media_item_id,position_ms,duration_ms,completed,updated_at)
            VALUES(1,{id},1000,10000,0,{id});
            INSERT INTO \
             item_metadata_links(id,media_item_id,provider_id,external_id,title,match_state)
            VALUES({id},{id},'tmdb','{id}','Item {id}','matched');"
        ))
        .unwrap();
    }
    conn.batch_execute(
        "INSERT INTO metadata_collections(id,provider_id,external_id,source_provider_id,
         source_external_id,relation_kind,locale_key,name)
         VALUES(1,'tmdb','1','tmdb','1','collection','en-US','Shared collection');
         INSERT INTO metadata_collection_items(collection_id,media_item_id,metadata_link_id)
         SELECT 1,id,id FROM media_items;",
    )
    .unwrap();
    let mut settings = media::list_library_settings(&mut conn).unwrap();
    assert!(settings.iter().all(|library| !library.exclude_from_home));
    assert!(
        !serde_json::from_str::<MediaLibrarySettings>(r#"{"name":"Legacy"}"#)
            .unwrap()
            .exclude_from_home
    );
    let before = media::get_media_home(&mut conn, Some(1), None).unwrap();
    assert_eq!(before.shelves[0].title, "Continue playing");
    assert_eq!(before.shelves[0].items.len(), 7);
    assert_eq!(before.collections[0].item_count, 7);

    // Keep one visible library; every other type uses the same persisted switch.
    for library in settings.iter_mut().skip(1) {
        library.exclude_from_home = true;
    }
    media::replace_library_settings(&mut conn, &settings).unwrap();
    assert_eq!(media::list_library_settings(&mut conn).unwrap(), settings);
    let home = media::get_media_home(&mut conn, Some(1), None).unwrap();
    assert!(
        home.shelves
            .iter()
            .flat_map(|shelf| &shelf.items)
            .all(|item| item.library_id == 1)
    );
    assert_eq!(home.shelves[0].items.len(), 1);
    assert_eq!(home.collections[0].item_ids, vec![1]);
    assert_eq!(home.collections[0].item_count, 1);
    for id in 1..=7 {
        let scoped = media::get_media_home(&mut conn, Some(1), Some(id)).unwrap();
        assert_eq!(scoped.shelves[0].items.len(), 1);
        assert_eq!(scoped.shelves[1].items.len(), 1);
        assert_eq!(
            scoped.shelves[0].title,
            match id {
                3 => "Continue listening",
                4 => "Continue reading",
                _ => "Continue watching",
            }
        );
        assert!(media::user_can_access_library(&mut conn, id, Some(1)).unwrap());
    }
    let summaries = media::get_persisted_library_summaries(&mut conn).unwrap();
    assert_eq!(summaries.len(), 7);
    assert!(
        summaries
            .iter()
            .skip(1)
            .all(|library| library.exclude_from_home)
    );
    assert_eq!(
        media::search_media_items_for_user_with_preferred_languages(
            &mut conn,
            Some(1),
            "Item",
            None,
            &[]
        )
        .unwrap()
        .len(),
        7
    );

    settings[0].exclude_from_home = true;
    media::replace_library_settings(&mut conn, &settings).unwrap();
    let empty = media::get_media_home(&mut conn, Some(1), None).unwrap();
    assert!(empty.shelves.iter().all(|shelf| shelf.items.is_empty()));
    assert!(empty.collections.is_empty());
}

#[test]
fn home_hides_excluded_radio_artwork_without_changing_saved_queue_order() {
    let mut conn = database();
    conn.batch_execute(
        "INSERT INTO media_libraries(id,name,path,kind,exclude_from_home) VALUES
        (1,'Visible','/visible','music',0),(2,'Excluded','/excluded','music',1);
        INSERT INTO \
         media_items(id,library_id,identity_key,item_type,display_title,playable,media_kind)
        VALUES(1,1,'visible','track','Visible song',1,'audio'),
        (2,2,'hidden','track','Excluded song',1,'audio');",
    )
    .unwrap();
    let hidden =
        media::playlists::create_radio(&mut conn, 1, "Excluded radio", &[2], None).unwrap();
    let mixed =
        media::playlists::create_radio(&mut conn, 1, "Mixed radio", &[2, 1, 2], Some(1)).unwrap();
    assert!(
        media::get_media_home(&mut conn, Some(1), None)
            .unwrap()
            .listening_queues
            .is_empty()
    );
    assert_eq!(
        media::get_media_home(&mut conn, Some(1), Some(2))
            .unwrap()
            .listening_queues
            .len(),
        2
    );
    media::playlists::update_queue(
        &mut conn,
        1,
        mixed.playlist.id,
        &media::playlists::QueueState {
            item_ids: vec![2, 1, 2],
            current_index: 1,
            position_ms: 1000,
            completed: false,
            shuffle: false,
            repeat_mode: "off".into(),
        },
    )
    .unwrap();
    let home = media::get_media_home(&mut conn, Some(1), None).unwrap();
    assert_eq!(home.listening_queues.len(), 1);
    assert_eq!(home.listening_queues[0].playlist.id, mixed.playlist.id);
    assert_eq!(
        home.listening_queues[0]
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![2, 1, 2]
    );
    assert_eq!(
        media::playlists::load(&mut conn, 1, hidden.playlist.id)
            .unwrap()
            .items
            .len(),
        1
    );
}
