use diesel::{
    Connection,
    connection::SimpleConnection,
};

use super::*;

fn fixture() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    conn.batch_execute(
        "INSERT INTO media_libraries (id,name,path,kind) VALUES
        (1,'Library','/media','movies'), (2,'Private','/private','music');
        INSERT INTO media_items (id,library_id,parent_id,identity_key,item_type,display_title) \
         VALUES
        (1,1,5,'show','show','Show'), (2,1,1,'season','season','Season'),
        (3,1,2,'episode','episode','Local episode'), (4,1,5,'artist','artist','Artist'),
        (5,1,NULL,'future','future-group','Local group'), (6,1,4,'album','album','Album'),
        (7,1,6,'track','track','Track'), (8,1,5,'movie','movie','Local movie'),
        (9,1,1,'deleted','season','Deleted'), (10,2,5,'private','movie','Private'),
        (11,1,9,'deleted-child','episode','Deleted descendant');
        UPDATE media_items SET deleted_at=1 WHERE id=9;
        INSERT INTO media_files (id,path,file_size,media_kind,container,duration_ms,
            bit_rate,width,height,video_codec,audio_codec,modified_at) VALUES
        (1,'/media/Show/episode.mkv',100,'video','matroska',1000,800,1920,1080,'h264','aac',42),
        (2,'/music/Artist/Album/01.flac',200,'audio','flac',2000,900,NULL,NULL,NULL,'flac',43),
        (3,'/media/Movie.mkv',300,'video',NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL),
        (4,'/overflow/Movie.mp4',400,'video',NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL),
        (5,'/private/secret.mkv',500,'video',NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL);
        INSERT INTO media_file_libraries
            (id,media_file_id,library_id,source_root_path,relative_path,media_item_id) VALUES
        (1,1,1,'/media','Show/episode.mkv',3),
        (2,2,1,'/music','Artist/Album/01.flac',7),
        (3,3,1,'/media','Movie.mkv',8),
        (4,4,1,'/overflow','Movie.mp4',8),
        (5,5,2,'/private','secret.mkv',10),
        (6,5,1,'/media','deleted.mkv',11),
        (7,5,1,'/media','removed.mkv',8),
        (8,5,2,'/private','corrupt-membership.mkv',8);
        UPDATE media_file_libraries SET deleted_at=1 WHERE id=7;
        UPDATE media_file_libraries SET missing_since=99 WHERE id=2;",
    )
    .unwrap();
    conn
}

#[test]
fn sources_cover_all_types_and_preserve_exact_paths_and_technical_facts() {
    let mut conn = fixture();
    for id in [1, 2, 3] {
        let page = get_item_source_files(&mut conn, id, 0, 25)
            .unwrap()
            .unwrap();
        assert_eq!(page.total_files, 1);
        let file = &page.files[0];
        assert_eq!(file.path, "/media/Show/episode.mkv");
        assert_eq!(file.source_root_path, "/media");
        assert_eq!(file.relative_path, "Show/episode.mkv");
        assert_eq!(file.display_title, "Local episode");
        assert_eq!(file.container.as_deref(), Some("matroska"));
        assert_eq!(file.width, Some(1920));
        assert_eq!(file.audio_codec.as_deref(), Some("aac"));
        assert_eq!(file.duration_ms, Some(1000));
        assert_eq!(file.bit_rate, Some(800));
        assert_eq!(file.modified_at, Some(42));
    }
    for id in [4, 6, 7] {
        let page = get_item_source_files(&mut conn, id, 0, 25)
            .unwrap()
            .unwrap();
        assert_eq!(page.total_files, 1);
        assert_eq!(page.missing_files, 1);
        assert_eq!(page.files[0].missing_since, Some(99));
    }
    let movie = get_item_source_files(&mut conn, 8, 0, 25).unwrap().unwrap();
    assert_eq!(movie.total_files, 2);
    assert_eq!(movie.total_size, 700);
    assert_eq!(movie.files[1].source_root_path, "/overflow");
    let future = get_item_source_files(&mut conn, 5, 0, 25).unwrap().unwrap();
    assert_eq!(future.catalog_title, "Local group");
    assert_eq!(future.total_files, 4);
    assert_eq!(future.total_size, 1000);
    assert_eq!(future.missing_files, 1);
    assert!(
        future
            .files
            .iter()
            .all(|file| !file.path.starts_with("/private"))
    );
    assert!(
        get_item_source_files(&mut conn, 9, 0, 25)
            .unwrap()
            .is_none()
    );
    assert!(
        get_item_source_files(&mut conn, 999, 0, 25)
            .unwrap()
            .is_none()
    );
}

#[test]
fn paging_is_bounded_stable_and_hierarchy_cycles_terminate() {
    let mut conn = fixture();
    conn.batch_execute("UPDATE media_items SET parent_id=3 WHERE id=5")
        .unwrap();
    let first = get_item_source_files(&mut conn, 5, -1, 0).unwrap().unwrap();
    let second = get_item_source_files(&mut conn, 5, 1, 1).unwrap().unwrap();
    assert_eq!((first.offset, first.limit, first.total_files), (0, 1, 4));
    assert_ne!(first.files[0].id, second.files[0].id);
    let all = get_item_source_files(&mut conn, 5, 0, 9999)
        .unwrap()
        .unwrap();
    assert_eq!(all.limit, 100);
    assert_eq!(all.files[0].id, first.files[0].id);
    assert_eq!(all.files[1].id, second.files[0].id);
    assert!(
        get_item_source_files(&mut conn, 5, 100, 25)
            .unwrap()
            .unwrap()
            .files
            .is_empty()
    );
}
