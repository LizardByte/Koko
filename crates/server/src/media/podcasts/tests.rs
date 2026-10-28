use super::*;
use crate::db::schema::{
    media_items as items,
    podcast_feeds as feeds,
    podcast_items,
    podcast_subscriptions as subscriptions,
    users,
};
use diesel::prelude::*;

const RSS: &str = include_str!("fixtures/feed.xml");

fn database() -> SqliteConnection {
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    crate::db::configure_sqlite_connection(&mut conn).unwrap();
    for id in 1..=3 {
        diesel::insert_into(users::table)
            .values((
                users::id.eq(id),
                users::username.eq(format!("user{id}")),
                users::password.eq("hash"),
                users::admin.eq(id == 3),
            ))
            .execute(&mut conn)
            .unwrap();
    }
    conn
}
fn save(
    conn: &mut SqliteConnection,
    user: i32,
    url: &str,
) -> i32 {
    storage::save(conn, Some(user), url, url, &feed::parse(RSS, url).unwrap()).unwrap()
}

#[test]
fn rss_retains_rich_metadata_and_publisher_season_numbers() {
    let value = feed::parse(RSS, "https://example.com/feed.xml").unwrap();
    assert_eq!(value.title, "Example show");
    assert_eq!(value.metadata.categories, ["Arts", "Books"]);
    assert_eq!(
        value.metadata.artwork_url.as_deref(),
        Some("https://example.com/cover.jpg")
    );
    assert_eq!(value.episodes.len(), 2);
    let episode = &value.episodes[0];
    assert_eq!(episode.guid, "stable-guid");
    assert_eq!(episode.season, Some(2));
    assert_eq!(episode.number, Some(1));
    assert_eq!(episode.duration_ms, Some(3_723_000));
    assert_eq!(episode.metadata.explicit, Some(false));
    assert_eq!(episode.metadata.author, value.metadata.author);
    assert_eq!(episode.metadata.show_type.as_deref(), Some("serial"));
    assert!(
        episode
            .metadata
            .description
            .as_deref()
            .unwrap()
            .contains("Full &amp; detailed")
    );
    assert_eq!(
        episode.metadata.transcripts[0].url,
        "https://example.com/one.vtt"
    );
    assert_eq!(episode.metadata.people, ["Speaker"]);
    assert!(episode.metadata.published_at.is_some());
    assert_eq!(value.episodes[1].season, None);
    assert_eq!(value.episodes[1].duration_ms, Some(45500));
}

#[test]
fn atom_and_invalid_feeds_are_handled_without_external_entities() {
    let atom = r#"<feed xmlns="http://www.w3.org/2005/Atom">
<title>Atom show</title>
<author>
<name>Author</name>
</author>
<entry>
<title>Entry</title>
<id>atom-id</id>
<published>2026-10-21T12:00:00Z</published>
<link rel="enclosure" href="audio.mp3" type="audio/mpeg"/>
</entry>
</feed>"#;
    let parsed = feed::parse(atom, "https://example.com/feed").unwrap();
    assert_eq!(parsed.episodes[0].url, "https://example.com/audio.mp3");
    assert_eq!(parsed.metadata.author.as_deref(), Some("Author"));
    for invalid in [
        "not XML",
        "<html><title>Site</title></html>",
        "<rss><channel/></rss>",
        r#"<!DOCTYPE rss [<!ENTITY x SYSTEM "file:///etc/passwd">]>
<rss>
<channel>
<title>&x;</title>
</channel>
</rss>"#,
    ] {
        assert!(feed::parse(invalid, "https://example.com").is_err());
    }
}

#[test]
fn duplicate_guids_and_unsafe_enclosures_do_not_create_extra_episodes() {
    let duplicate = RSS.replace(
        "<item><title>No enclosure</title></item>",
        r#"<item>
<title>Duplicate</title>
<guid>stable-guid</guid>
<enclosure url="https://example.com/another.mp3" type="audio/mpeg"/>
</item>
<item>
<title>Unsafe</title>
<enclosure url="file:///etc/passwd" type="audio/mpeg"/>
</item>"#,
    );
    assert_eq!(
        feed::parse(&duplicate, "https://example.com/feed")
            .unwrap()
            .episodes
            .len(),
        2
    );
    for invalid in [
        "-1", "NaN", "inf", "1:2:3:4", "abc",
    ] {
        assert_eq!(feed::duration(invalid), None);
    }
}

#[test]
fn subscriptions_reuse_content_but_do_not_grant_other_users_or_admins_access() {
    let mut conn = database();
    let root = save(&mut conn, 1, "https://example.com/feed");
    assert_eq!(save(&mut conn, 2, "https://example.com/feed"), root);
    assert_eq!(save(&mut conn, 1, "https://example.com/feed"), root);
    assert_eq!(
        feeds::table.count().get_result::<i64>(&mut conn).unwrap(),
        1
    );
    assert_eq!(
        subscriptions::table
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap(),
        2
    );
    assert_eq!(
        items::table.count().get_result::<i64>(&mut conn).unwrap(),
        5
    );
    let ids = items::table
        .select(items::id)
        .load::<i32>(&mut conn)
        .unwrap();
    for id in ids {
        assert!(can_access(&mut conn, id, Some(1)).unwrap());
        assert!(can_access(&mut conn, id, Some(2)).unwrap());
        assert!(!can_access(&mut conn, id, Some(3)).unwrap());
        assert!(!can_access(&mut conn, id, None).unwrap());
    }
    assert!(subscribed(&mut conn, 3).unwrap().is_empty());
    storage::unsubscribe(&mut conn, 1, root).unwrap();
    assert!(!can_access(&mut conn, root, Some(1)).unwrap());
    assert!(can_access(&mut conn, root, Some(2)).unwrap());
    assert!(storage::feed_for_user(&mut conn, 1, root).is_err());
}

#[test]
fn redirected_aliases_share_one_feed() {
    let mut conn = database();
    let feed = feed::parse(RSS, "https://example.com/new").unwrap();
    let root = storage::save(
        &mut conn,
        Some(1),
        "https://example.com/old",
        "https://example.com/new",
        &feed,
    )
    .unwrap();
    assert_eq!(
        storage::subscribe_existing(&mut conn, 2, "https://example.com/old").unwrap(),
        Some(root)
    );
    assert_eq!(
        storage::save(
            &mut conn,
            Some(2),
            "https://example.com/new",
            "https://example.com/new",
            &feed
        )
        .unwrap(),
        root
    );
    assert_eq!(
        feeds::table.count().get_result::<i64>(&mut conn).unwrap(),
        1
    );
}

#[test]
fn search_home_and_subscriptions_filter_each_feed_within_the_shared_library() {
    let mut conn = database();
    let first = save(&mut conn, 1, "https://example.com/one");
    let second = save(&mut conn, 2, "https://example.com/two");
    assert_ne!(first, second);
    assert_eq!(
        subscribed(&mut conn, 1)
            .unwrap()
            .iter()
            .map(|i| i.id)
            .collect::<Vec<_>>(),
        [first]
    );
    let result = crate::media::search_media_items_for_user_with_preferred_languages(
        &mut conn,
        Some(1),
        "episode",
        None,
        &[],
    )
    .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].podcast.as_ref().unwrap().podcast_id, first);
    let home = crate::media::get_media_home(&mut conn, Some(1), None).unwrap();
    for shelf in home.shelves.iter().filter(|s| {
        [
            "recently_added",
            "recommended",
        ]
        .contains(&s.id.as_str())
    }) {
        assert_eq!(shelf.items.len(), 1);
        assert_eq!(shelf.items[0].item_type, "podcast");
    }
    assert!(
        home.shelves
            .iter()
            .flat_map(|s| &s.items)
            .all(|i| i.podcast.as_ref().unwrap().podcast_id == first)
    );
    assert!(
        crate::media::get_media_home(&mut conn, Some(3), None)
            .unwrap()
            .shelves
            .iter()
            .all(|s| s.items.is_empty())
    );
}

#[test]
fn refresh_preserves_ids_history_and_episodes_omitted_by_a_rolling_feed() {
    let mut conn = database();
    let root = save(&mut conn, 1, "https://example.com/feed");
    let episode = items::table
        .filter(items::display_title.eq("First episode"))
        .select(items::id)
        .first::<i32>(&mut conn)
        .unwrap();
    crate::media::upsert_playback_progress(&mut conn, 1, episode, 1000, Some(3_723_000), false)
        .unwrap();
    let changed = RSS.replace("one.mp3", "one-new.mp3").replace(
        "<itunes:season>2</itunes:season>",
        "<itunes:season>3</itunes:season>",
    );
    let mut parsed = feed::parse(&changed, "https://example.com/feed").unwrap();
    parsed.episodes.truncate(1);
    storage::save(
        &mut conn,
        None,
        "https://example.com/feed",
        "https://example.com/feed",
        &parsed,
    )
    .unwrap();
    assert_eq!(
        items::table
            .find(episode)
            .select(items::season_number)
            .first::<Option<i32>>(&mut conn)
            .unwrap(),
        Some(3)
    );
    assert_eq!(
        podcast_items::table
            .find(episode)
            .select(podcast_items::enclosure_url)
            .first::<Option<String>>(&mut conn)
            .unwrap()
            .as_deref(),
        Some("https://cdn.example.com/one-new.mp3")
    );
    assert_eq!(
        crate::media::get_user_playback_progress(&mut conn, Some(1), episode)
            .unwrap()
            .unwrap()
            .position_ms,
        1000
    );
    assert_eq!(
        items::table
            .filter(items::item_type.eq("podcast_episode"))
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap(),
        2
    );
    assert_eq!(
        feeds::table
            .find(root)
            .select(feeds::last_error)
            .first::<Option<String>>(&mut conn)
            .unwrap(),
        None
    );
}

#[test]
fn metadata_detail_playback_and_configuration_preserve_the_managed_library() {
    let mut conn = database();
    let root = save(&mut conn, 1, "https://example.com/feed");
    let detail = crate::media::get_media_item(&mut conn, root, "")
        .unwrap()
        .unwrap();
    assert_eq!(detail.children.len(), 2);
    assert!(detail.poster_url.unwrap().ends_with("cover.jpg"));
    assert_eq!(
        detail.podcast.unwrap().feed_url.as_deref(),
        Some("https://example.com/feed")
    );
    let episode = items::table
        .filter(items::display_title.eq("First episode"))
        .select(items::id)
        .first::<i32>(&mut conn)
        .unwrap();
    let decision = crate::media::get_playback_decision(&mut conn, episode, None)
        .unwrap()
        .unwrap();
    assert!(decision.can_direct_play);
    assert_eq!(decision.mime_type.as_deref(), Some("audio/mpeg"));
    assert!(
        decision
            .stream_url
            .unwrap()
            .starts_with("/api/v1/podcasts/episodes/")
    );
    assert!(
        crate::media::list_library_settings(&mut conn)
            .unwrap()
            .is_empty()
    );
    crate::media::replace_library_settings(&mut conn, &[]).unwrap();
    assert!(can_access(&mut conn, root, Some(1)).unwrap());
    storage::record_check(&mut conn, root, Some("Publisher unavailable".into())).unwrap();
    assert_eq!(
        crate::media::get_media_item(&mut conn, root, "")
            .unwrap()
            .unwrap()
            .podcast
            .unwrap()
            .last_error
            .as_deref(),
        Some("Publisher unavailable")
    );
}

#[test]
fn refresh_queue_only_includes_subscribed_feeds_after_the_interval() {
    let mut conn = database();
    let first = save(&mut conn, 1, "https://example.com/one");
    let second = save(&mut conn, 2, "https://example.com/two");
    assert!(storage::due(&mut conn).unwrap().is_empty());
    diesel::update(feeds::table)
        .set(feeds::checked_at.eq(0))
        .execute(&mut conn)
        .unwrap();
    storage::unsubscribe(&mut conn, 2, second).unwrap();
    assert_eq!(
        storage::due(&mut conn).unwrap(),
        [(first, "https://example.com/one".into())]
    );
}

#[test]
fn urls_reject_credentials_and_non_web_schemes_and_normalize_fragments() {
    assert_eq!(
        http::normalize_url(" HTTPS://Example.COM/feed#episode ").unwrap(),
        "https://example.com/feed"
    );
    for url in [
        "file:///etc/passwd",
        "ftp://example.com/feed",
        "https://user:password@example.com/feed",
        "garbage",
    ] {
        assert!(http::normalize_url(url).is_err());
    }
}

#[test]
fn public_network_policy_blocks_private_reserved_and_mapped_addresses() {
    for address in [
        "127.0.0.1",
        "10.0.0.1",
        "172.16.0.1",
        "192.168.0.1",
        "169.254.169.254",
        "100.100.100.200",
        "0.0.0.0",
        "198.18.0.1",
        "192.0.2.1",
        "224.0.0.1",
        "255.255.255.255",
        "::1",
        "::",
        "::ffff:127.0.0.1",
        "fd00::1",
        "fe80::1",
        "2001:db8::1",
        "2002:7f00:1::",
    ] {
        assert!(!http::public_address(address.parse().unwrap()), "{address}");
    }
    for address in [
        "1.1.1.1",
        "8.8.8.8",
        "2606:4700:4700::1111",
    ] {
        assert!(http::public_address(address.parse().unwrap()), "{address}");
    }
}

#[tokio::test]
async fn private_feed_requests_fail_before_connecting() {
    assert!(
        http::request("http://127.0.0.1/feed", None)
            .await
            .unwrap_err()
            .contains("public internet")
    );
}

#[test]
fn directory_results_require_a_public_feed_and_never_write_to_storage() {
    let response = serde_json::json!({"results": [
        {"collectionId": 123, "collectionName": "Show",
             "feedUrl": "https://example.com/feed", "artistName": "Author",
             "artworkUrl600": "https://example.com/cover.jpg"},
        {"collectionId": 456, "collectionName": "No public feed"},
        {"collectionId": 789, "collectionName": "Unsafe", "feedUrl": "file:///local"}
    ]});
    let results = discovery::results(&response);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "123");
}

#[test]
fn unseasoned_episodes_use_publication_year_without_changing_numbered_seasons() {
    let mut conn = database();
    let url = "https://example.com/feed";
    let mut parsed = feed::parse(RSS, url).unwrap();
    parsed.episodes[1].metadata.published_at = Some(1_735_689_600); // 2025-01-01 UTC.
    let root = storage::save(&mut conn, Some(1), url, url, &parsed).unwrap();
    let detail = crate::media::get_media_item(&mut conn, root, "")
        .unwrap()
        .unwrap();
    assert_eq!(detail.children.len(), 2);
    let year = detail
        .children
        .iter()
        .find(|item| item.display_title == "2025")
        .unwrap();
    assert_eq!(year.item_type, "podcast_season");
    assert_eq!(year.season_number, None);
    assert_eq!(
        year.podcast.as_ref().unwrap().season_kind,
        Some(PodcastSeasonKind::Year)
    );
    assert_eq!(year.podcast.as_ref().unwrap().season_year, Some(2025));
    let numbered = detail
        .children
        .iter()
        .find(|item| item.season_number == Some(2))
        .unwrap();
    assert_eq!(
        numbered.podcast.as_ref().unwrap().season_kind,
        Some(PodcastSeasonKind::Number)
    );
    let year_id = year.id;
    let episode = items::table
        .filter(items::parent_id.eq(year_id))
        .select(items::id)
        .first::<i32>(&mut conn)
        .unwrap();
    crate::media::upsert_playback_progress(&mut conn, 1, episode, 1000, Some(45500), false)
        .unwrap();
    // Reproduce the old layout, then reconcile without contacting the publisher.
    diesel::update(items::table.find(episode))
        .set(items::parent_id.eq(root))
        .execute(&mut conn)
        .unwrap();
    diesel::delete(items::table.find(year_id))
        .execute(&mut conn)
        .unwrap();
    storage::reconcile_year_seasons(&mut conn).unwrap();
    storage::reconcile_year_seasons(&mut conn).unwrap();
    let detail = crate::media::get_media_item(&mut conn, root, "")
        .unwrap()
        .unwrap();
    let year = detail
        .children
        .iter()
        .find(|item| item.display_title == "2025")
        .unwrap();
    assert_eq!(year.child_count, 1);
    assert_eq!(
        items::table
            .find(episode)
            .select(items::parent_id)
            .first::<Option<i32>>(&mut conn)
            .unwrap(),
        Some(year.id)
    );
    assert_eq!(
        crate::media::get_user_playback_progress(&mut conn, Some(1), episode)
            .unwrap()
            .unwrap()
            .position_ms,
        1000
    );
}

#[test]
fn imports_defer_archives_but_refresh_queues_new_episodes_and_preserves_saved_copies() {
    let mut conn = database();
    let url = "https://example.com/feed";
    let root = save(&mut conn, 1, url);
    let ids = storage::episode_ids(&mut conn, root).unwrap();
    for id in &ids {
        assert!(
            downloads::stored(&mut conn, *id)
                .unwrap()
                .local_download
                .is_none()
        );
    }
    let saved = downloads::LocalDownload {
        state: DownloadState::Ready,
        path: Some("saved-episode.mp3".into()),
        size_bytes: Some(4),
        duration_ms: Some(120_000),
        attempted_at: 10,
        error: None,
    };
    downloads::set_record(&mut conn, ids[0], saved).unwrap();
    let updated = RSS.replace("stable-guid", "new-episode-guid");
    storage::save(
        &mut conn,
        None,
        url,
        url,
        &feed::parse(&updated, url).unwrap(),
    )
    .unwrap();
    let all = storage::episode_ids(&mut conn, root).unwrap();
    let new = *all.iter().find(|id| !ids.contains(id)).unwrap();
    assert_eq!(
        downloads::stored(&mut conn, new)
            .unwrap()
            .local_download
            .unwrap()
            .state,
        DownloadState::Queued
    );
    storage::save(&mut conn, None, url, url, &feed::parse(RSS, url).unwrap()).unwrap();
    let stored = downloads::stored(&mut conn, ids[0])
        .unwrap()
        .local_download
        .unwrap();
    assert_eq!(stored.state, DownloadState::Ready);
    assert_eq!(stored.path.unwrap().to_str(), Some("saved-episode.mp3"));
    let public = crate::media::get_media_item(&mut conn, ids[0], "")
        .unwrap()
        .unwrap();
    let public = serde_json::to_string(&public).unwrap();
    assert!(public.contains("ready"));
    assert!(!public.contains("saved-episode.mp3"));
    assert!(!public.contains("local_download"));
}

#[test]
fn playback_order_is_private_per_show_and_applies_across_seasons_with_undated_last() {
    let mut conn = database();
    let url = "https://example.com/feed";
    let mut feed = feed::parse(RSS, url).unwrap();
    feed.episodes[1].metadata.published_at = Some(1_735_689_600);
    let root = storage::save(&mut conn, Some(1), url, url, &feed).unwrap();
    storage::subscribe_existing(&mut conn, 2, url).unwrap();
    let oldest = preferences::episodes(&mut conn, 1, root).unwrap();
    assert_eq!(oldest.len(), 2);
    assert!(
        oldest[0].podcast.as_ref().unwrap().published_at
            < oldest[1].podcast.as_ref().unwrap().published_at
    );
    preferences::set(&mut conn, 1, root, PlaybackOrder::NewestFirst).unwrap();
    assert_eq!(
        preferences::episodes(&mut conn, 1, root).unwrap()[0].id,
        oldest[1].id
    );
    assert_eq!(
        preferences::episodes(&mut conn, 2, root).unwrap()[0].id,
        oldest[0].id
    );
    assert!(preferences::set(&mut conn, 3, root, PlaybackOrder::NewestFirst).is_err());
    let other = save(&mut conn, 1, "https://example.com/other");
    assert_eq!(
        preferences::get(&mut conn, 1, other).unwrap(),
        PlaybackOrder::OldestFirst
    );
    let mut detail = crate::media::get_media_item(&mut conn, root, "")
        .unwrap()
        .unwrap();
    crate::media::apply_user_playback_context_to_detail(&mut conn, Some(1), &mut detail).unwrap();
    assert_eq!(
        detail.playback_target.as_ref().unwrap().item_id,
        oldest[1].id
    );
    crate::media::upsert_playback_progress(&mut conn, 1, oldest[0].id, 1000, Some(45500), false)
        .unwrap();
    crate::media::apply_user_playback_context_to_detail(&mut conn, Some(1), &mut detail).unwrap();
    assert_eq!(
        detail.playback_target.as_ref().unwrap().item_id,
        oldest[0].id
    );
    let mut undated = oldest[0].clone();
    undated.podcast.as_mut().unwrap().published_at = None;
    let mut rows = vec![undated, oldest[1].clone()];
    for order in [
        PlaybackOrder::OldestFirst,
        PlaybackOrder::NewestFirst,
    ] {
        preferences::sort(&mut rows, order);
        assert!(rows[0].podcast.as_ref().unwrap().published_at.is_some());
    }
}

#[test]
fn catalog_metadata_and_progress_queries_handle_more_than_sqlites_parameter_limit() {
    let mut conn = database();
    let root = save(&mut conn, 1, "https://example.com/large");
    let ids = vec![root; 33_000];
    assert!(
        crate::media::preferred_metadata_links_by_item_id(&mut conn, &ids, &[])
            .unwrap()
            .is_empty()
    );
    assert!(
        crate::media::playback_progress_by_item_id(&mut conn, Some(1), &ids)
            .unwrap()
            .is_empty()
    );
    assert!(
        crate::media::audiobooks::load(&mut conn, &ids)
            .unwrap()
            .is_empty()
    );
    assert!(
        !crate::media::get_media_home(&mut conn, Some(1), None)
            .unwrap()
            .shelves
            .is_empty()
    );
}

#[test]
fn season_pages_are_bounded_newest_first_and_keep_the_oldest_first_playback_queue() {
    let mut conn = database();
    let url = "https://example.com/long";
    let mut feed = feed::parse(RSS, url).unwrap();
    feed.episodes.clear();
    for index in 0..125 {
        let mut episode = feed::parse(RSS, url).unwrap().episodes.remove(0);
        episode.guid = format!("episode-{index}");
        episode.metadata.published_at = Some(1_735_689_600 + index);
        episode.title = format!("Episode {index}");
        feed.episodes.push(episode);
    }
    let root = storage::save(&mut conn, Some(1), url, url, &feed).unwrap();
    let show = crate::media::get_media_item(&mut conn, root, "")
        .unwrap()
        .unwrap();
    let season = show.children[0].id;
    let detail = crate::media::get_media_item(&mut conn, season, "")
        .unwrap()
        .unwrap();
    assert_eq!(detail.children.len(), 50);
    assert_eq!(detail.children[0].display_title, "Episode 124");
    let mut ids = Vec::new();
    for offset in [0, 50, 100] {
        let page = storage::episode_page(&mut conn, season, offset, 50, &[]).unwrap();
        assert_eq!(page.total, 125);
        assert_eq!(page.has_more, offset < 100);
        ids.extend(page.items.iter().map(|item| item.id));
    }
    assert_eq!(ids.len(), 125);
    assert_eq!(
        ids.iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        125
    );
    let playback = preferences::episodes(&mut conn, 1, root).unwrap();
    assert_eq!(playback[0].display_title, "Episode 0");
}

#[test]
fn home_and_library_status_work_with_a_catalog_above_sqlites_bind_limit() {
    use crate::db::schema::media_libraries as libraries;
    let mut conn = database();
    diesel::insert_into(libraries::table)
        .values((
            libraries::id.eq(1),
            libraries::name.eq("Large library"),
            libraries::path.eq("/movies"),
            libraries::kind.eq("movies"),
        ))
        .execute(&mut conn)
        .unwrap();
    conn.immediate_transaction::<_, diesel::result::Error, _>(|conn| {
        for batch in 0..331 {
            let values = (0..100)
                .map(|index| {
                    let index = batch * 100 + index;
                    (
                        items::library_id.eq(1),
                        items::identity_key.eq(format!("large-{index}")),
                        items::item_type.eq("movie"),
                        items::display_title.eq(format!("Movie {index}")),
                    )
                })
                .collect::<Vec<_>>();
            diesel::insert_into(items::table)
                .values(&values)
                .execute(conn)?;
        }
        Ok(())
    })
    .unwrap();
    assert!(
        !crate::media::get_media_home(&mut conn, Some(3), None)
            .unwrap()
            .shelves
            .is_empty()
    );
    assert_eq!(
        crate::media::get_persisted_library_summaries(&mut conn)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn continue_playing_selects_one_episode_per_podcast_and_advances_across_seasons() {
    use crate::db::schema::playback_progress as progress;
    let mut conn = database();
    let url = "https://example.com/continuation";
    let mut feed = feed::parse(RSS, url).unwrap();
    feed.episodes.clear();
    for index in 0..4 {
        let mut episode = feed::parse(RSS, url).unwrap().episodes.remove(0);
        episode.guid = format!("continue-{index}");
        episode.title = format!("Episode {index}");
        episode.season = Some(index / 2 + 1);
        episode.metadata.published_at = Some(1_735_689_600 + index as i64);
        feed.episodes.push(episode);
    }
    let root = storage::save(&mut conn, Some(1), url, url, &feed).unwrap();
    storage::subscribe_existing(&mut conn, 2, url).unwrap();
    let episodes = preferences::episodes(&mut conn, 1, root).unwrap();
    for (index, position) in [(0, 1000), (1, 2000)] {
        crate::media::upsert_playback_progress(
            &mut conn,
            1,
            episodes[index].id,
            position,
            Some(9000),
            false,
        )
        .unwrap();
        diesel::update(
            progress::table
                .filter(progress::user_id.eq(1))
                .filter(progress::media_item_id.eq(episodes[index].id)),
        )
        .set(progress::updated_at.eq(index as i64 + 10))
        .execute(&mut conn)
        .unwrap();
    }
    let continuation = |conn: &mut SqliteConnection, user| {
        crate::media::get_media_home(conn, Some(user), None)
            .unwrap()
            .shelves
            .into_iter()
            .find(|shelf| shelf.id == "continue_watching")
            .unwrap()
            .items
    };
    let current = continuation(&mut conn, 1);
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].id, episodes[1].id);
    assert_eq!(current[0].display_title, "Example show");
    assert_eq!(current[0].display_subtitle.as_deref(), Some("Episode 1"));
    assert!(continuation(&mut conn, 2).is_empty());
    crate::media::upsert_playback_progress(&mut conn, 1, episodes[1].id, 9000, Some(9000), true)
        .unwrap();
    let next = continuation(&mut conn, 1);
    assert_eq!(next.len(), 1);
    assert_eq!(next[0].id, episodes[2].id);
    assert_ne!(next[0].parent_id, episodes[1].parent_id);
    preferences::set(&mut conn, 1, root, PlaybackOrder::NewestFirst).unwrap();
    assert_eq!(continuation(&mut conn, 1)[0].id, episodes[0].id);
    preferences::set(&mut conn, 1, root, PlaybackOrder::OldestFirst).unwrap();
    crate::media::upsert_playback_progress(&mut conn, 1, episodes[3].id, 9000, Some(9000), true)
        .unwrap();
    assert!(continuation(&mut conn, 1).is_empty());
}

#[test]
fn personal_home_visibility_does_not_change_other_users_subscriptions_search_or_podcast_home() {
    let mut conn = database();
    let root = save(&mut conn, 1, "https://example.com/preferences");
    storage::subscribe_existing(&mut conn, 2, "https://example.com/preferences").unwrap();
    assert!(
        preferences::user_settings(&mut conn, 1)
            .unwrap()
            .show_on_home
    );
    preferences::save_user_settings(
        &mut conn,
        1,
        &PodcastUserSettings {
            show_on_home: false,
        },
    )
    .unwrap();
    let home = crate::media::get_media_home(&mut conn, Some(1), None).unwrap();
    assert!(
        home.shelves
            .iter()
            .all(|shelf| shelf.items.iter().all(|item| item.podcast.is_none()))
    );
    assert!(
        preferences::user_settings(&mut conn, 2)
            .unwrap()
            .show_on_home
    );
    assert!(
        crate::media::get_media_home(&mut conn, Some(2), None)
            .unwrap()
            .shelves
            .iter()
            .any(|shelf| !shelf.items.is_empty())
    );
    let library = items::table
        .find(root)
        .select(items::library_id)
        .first::<i32>(&mut conn)
        .unwrap();
    assert!(
        crate::media::get_media_home(&mut conn, Some(1), Some(library))
            .unwrap()
            .shelves
            .iter()
            .any(|shelf| !shelf.items.is_empty())
    );
    assert_eq!(storage::subscribed(&mut conn, 1).unwrap()[0].id, root);
    assert!(
        !crate::media::search_media_items_for_user_with_preferred_languages(
            &mut conn,
            Some(1),
            "Example",
            None,
            &[]
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn large_feed_archives_above_the_old_ten_megabyte_limit_are_supported() {
    let padding = " ".repeat(10 * 1024 * 1024);
    let xml = RSS.replacen("<channel>", &format!("<channel><!--{padding}-->"), 1);
    assert!(xml.len() > 10 * 1024 * 1024 && xml.len() < MAX_FEED_BYTES);
    let feed = feed::parse(&xml, "https://example.com/large-archive").unwrap();
    assert_eq!(feed.episodes.len(), 2);
}
