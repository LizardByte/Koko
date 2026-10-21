//! Recording identity, file-boundary timing, and HTTP contracts using synthetic LibriVox fixtures.

use super::super::audiosilo::tests::{
    mock,
    mock_status,
};
use super::*;
use serde_json::json;

fn project() -> serde_json::Value {
    json!({
        "id": "6112", "title": "How I Filmed the War", "copyright_year": "1920",
        "authors": [{"first_name": "Geoffrey H.", "last_name": "Malins"}],
        "language": "English", "num_sections": "2", "totaltimesecs": 125,
        "description": "A synthetic fixture for provider tests.",
        "coverart_jpg": "https://archive.org/covers/6112.jpg", "genres": [{"name": "Memoirs"}],
        "sections": [
            {"section_number": "2", "title": "Part II", "playtime": "65",
                "listen_url": "https://archive.org/book/war_02_malins_64kb.mp3",
                "readers": [{"display_name": "R. S. Steinberg"}]},
            {"section_number": 1, "title": "Part I, Chapters I and II", "playtime": 60,
                "listen_url": "https://archive.org/book/war_01_malins_64kb.mp3",
                "readers": [{"display_name": "R. S. Steinberg"}]}
        ]
    })
}

fn local() -> AudiobookMetadata {
    AudiobookMetadata {
        title: "How I Filmed the War".into(),
        author: "Geoffrey H Malins".into(),
        ..Default::default()
    }
}

fn files() -> Vec<RecordingFile> {
    vec![
        RecordingFile {
            path: "Book/war_01_malins_128kb.mp3".into(),
            duration_ms: Some(60500),
        },
        RecordingFile {
            path: "Book/war_02_malins_128kb.mp3".into(),
            duration_ms: Some(65500),
        },
    ]
}

#[tokio::test]
async fn search_exposes_work_and_recording_with_author_filter_and_bounded_extended_fields() {
    let (root, worker) = mock(vec![(
        "format=json&extended=1&coverart=1&limit=50",
        json!({"books": [project()]}),
    )]);
    let found = TEST_API_ROOT
        .scope(
            root,
            LibriVoxMetadataProvider.search_with_artist(
                &MetadataSettings::default(),
                "How I Filmed the War",
                Some("audiobook"),
                Some("Geoffrey H Malins"),
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].external_id, "6112");
    assert_eq!(found[1].external_id, "6112/recording");
    assert!(found[1].title.contains("R. S. Steinberg"));
    assert_eq!(found[1].release_year, None);
    let (root, worker) = mock(vec![("title=How", json!({"books": [project()]}))]);
    assert!(
        TEST_API_ROOT
            .scope(
                root,
                LibriVoxMetadataProvider.search_with_artist(
                    &MetadataSettings::default(),
                    "How I Filmed the War",
                    Some("audiobook"),
                    Some("Another Author")
                )
            )
            .await
            .unwrap()
            .is_empty()
    );
    worker.join().unwrap();
}

#[tokio::test]
async fn automatic_matching_uses_original_files_and_runtime_without_guessing_a_narrator() {
    let (root, worker) = mock(vec![("title=How", json!({"books": [project()]}))]);
    let found = TEST_API_ROOT
        .scope(root, guess(&local(), Some(126000), &files()))
        .await
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_eq!(found.external_id, "6112/recording");
}

#[tokio::test]
async fn folder_hints_are_tried_after_a_tag_query_fails_and_titles_are_literal_searches() {
    let (root, worker) = mock_status(vec![
        (
            "title=Wrong+%5C%5Btitle",
            503,
            json!({"error":"unavailable"}),
        ),
        ("title=How", 200, json!({"books":[project()]})),
    ]);
    let metadata = AudiobookMetadata {
        title: "Wrong [title".into(),
        author: "Wrong Author".into(),
        folder_title: local().title,
        folder_author: local().author,
        ..Default::default()
    };
    let found = TEST_API_ROOT
        .scope(root, guess(&metadata, Some(126000), &files()))
        .await
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_eq!(found.external_id, "6112/recording");
}

#[tokio::test]
async fn automatic_matching_preserves_folder_hints_and_leaves_ambiguous_versions_unconfirmed() {
    let mut second = project();
    second["id"] = json!(7000);
    let (root, worker) = mock(vec![("title=How", json!({"books": [project(), second]}))]);
    let metadata = AudiobookMetadata {
        folder_title: local().title,
        folder_author: local().author,
        ..Default::default()
    };
    let found = TEST_API_ROOT
        .scope(root, guess(&metadata, Some(126000), &files()))
        .await
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    assert_eq!(found.external_id, "6112");
    assert!(found.title.contains("Book details only"));
}

#[test]
fn filename_runtime_narrator_and_missing_file_mismatches_do_not_confirm_a_recording() {
    let book: Book = serde_json::from_value(project()).unwrap();
    let mut metadata = local();
    assert!(!recording_matches(&book, &metadata, Some(800000), &files()));
    metadata.narrator = "Wrong reader".into();
    assert!(!recording_matches(&book, &metadata, Some(126000), &files()));
    metadata.narrator.clear();
    let mut parts = files();
    parts[0].path = "renamed.mp3".into();
    assert!(!recording_matches(&book, &metadata, Some(126000), &parts));
    metadata.narrator = "R S Steinberg".into();
    assert!(recording_matches(&book, &metadata, Some(126000), &parts));
    parts[0].duration_ms = None;
    assert!(!recording_matches(&book, &metadata, Some(126000), &parts));
    assert!(!recording_matches(&book, &metadata, Some(126000), &[]));
}

#[tokio::test]
async fn selected_project_keeps_its_cover_reader_and_available_section_boundaries() {
    let (root, worker) = mock(vec![("id=6112", json!({"books": [project()]}))]);
    let snapshot = TEST_API_ROOT
        .scope(
            root,
            LibriVoxMetadataProvider.fetch_snapshot(
                &MetadataSettings::default(),
                "6112/recording",
                "audiobook",
                true,
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    let metadata = recording_metadata(snapshot.provider_payload_json.as_deref().unwrap()).unwrap();
    assert_eq!(metadata.narrator, "R. S. Steinberg");
    assert_eq!(metadata.chapters.len(), 2);
    assert_eq!(metadata.chapters[0].title, "Part I, Chapters I and II");
    assert_eq!(metadata.chapters[0].start_ms, 0);
    assert_eq!(metadata.chapters[1].start_ms, 60000);
    assert_eq!(metadata.chapters[1].end_ms, 125000);
    assert_eq!(
        metadata.chapter_files,
        [
            "war_01_malins_64kb.mp3",
            "war_02_malins_64kb.mp3"
        ]
    );
    assert_eq!(
        snapshot.artwork_url.as_deref(),
        Some("https://archive.org/covers/6112.jpg")
    );
    assert_eq!(
        metadata.recording.unwrap().provider_id,
        MetadataProviderId::LibriVox
    );
    assert_eq!(snapshot.release_year, None); // The text's copyright year is not the recording date.
    assert_eq!(
        LibriVoxMetadataProvider.metadata_details(&snapshot).genres,
        ["Memoirs"]
    );
}

#[tokio::test]
async fn work_only_matches_keep_recording_facts_separate_and_choices_keep_every_project() {
    let (root, worker) = mock(vec![("id=6112", json!({"books": [project()]}))]);
    let snapshot = TEST_API_ROOT
        .scope(
            root,
            LibriVoxMetadataProvider.fetch_snapshot(
                &MetadataSettings::default(),
                "6112",
                "audiobook",
                false,
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    let metadata = recording_metadata(snapshot.provider_payload_json.as_deref().unwrap()).unwrap();
    assert!(metadata.recording.is_none());
    assert!(metadata.narrator.is_empty());
    assert!(metadata.chapters.is_empty());
    assert!(metadata.chapter_files.is_empty());
    assert!(snapshot.artwork_url.is_none());
    let mut second = project();
    second["id"] = json!(7000);
    let (root, worker) = mock(vec![("title=How", json!({"books": [project(), second]}))]);
    let choices = TEST_API_ROOT
        .scope(root, recording_choices(&local()))
        .await
        .unwrap();
    worker.join().unwrap();
    assert_eq!(choices.len(), 2);
    assert_ne!(choices[0].external_id, choices[1].external_id);
    assert!(choices.iter().all(|choice| choice.artwork_url.is_some()));
}

#[test]
fn missing_invalid_or_incomplete_section_timings_do_not_create_chapter_offsets() {
    for value in [
        json!("0"),
        json!("invalid"),
        json!("9223372036854775807"),
    ] {
        let mut source = project();
        source["sections"][0]["playtime"] = value;
        assert!(chapters(&serde_json::from_value(source).unwrap()).is_empty());
    }
    let mut source = project();
    source["num_sections"] = json!(3);
    assert!(chapters(&serde_json::from_value(source).unwrap()).is_empty());
}

#[tokio::test]
async fn recorded_real_project_preserves_the_20_sections_and_eight_hour_narration() {
    // Factual fields captured from the public API, without an external network dependency.
    let source: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/6112.json")).unwrap();
    let (root, worker) = mock(vec![("id=6112", json!({"books": [source]}))]);
    let snapshot = TEST_API_ROOT
        .scope(
            root,
            LibriVoxMetadataProvider.fetch_snapshot(
                &MetadataSettings::default(),
                "6112/recording",
                "audiobook",
                false,
            ),
        )
        .await
        .unwrap();
    worker.join().unwrap();
    let metadata = recording_metadata(snapshot.provider_payload_json.as_deref().unwrap()).unwrap();
    assert_eq!(metadata.narrator, "R. S. Steinberg");
    assert_eq!(metadata.chapters.len(), 20);
    assert_eq!(metadata.chapter_files.len(), 20);
    assert_eq!(metadata.chapters.last().unwrap().end_ms, 30198000);
    assert_eq!(metadata.recording.unwrap().runtime_ms, Some(30198000));
    assert!(
        snapshot
            .artwork_url
            .unwrap()
            .ends_with("How_Filmed_War_1303.jpg")
    );
}

#[tokio::test]
async fn invalid_ids_and_unrelated_media_types_do_not_send_requests() {
    for id in [
        "0",
        "../6112",
        "6112/other",
        "6112/recording/more",
        "999999999999999",
    ] {
        assert!(
            LibriVoxMetadataProvider
                .fetch_snapshot(&MetadataSettings::default(), id, "audiobook", false)
                .await
                .is_err()
        );
    }
    assert!(
        LibriVoxMetadataProvider
            .search(&MetadataSettings::default(), "Book", Some("author"))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        LibriVoxMetadataProvider
            .search(&MetadataSettings::default(), "", Some("audiobook"))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn missing_projects_and_http_failures_are_distinct_from_empty_searches() {
    let (root, worker) = mock(vec![("id=6112", json!({"books": []}))]);
    assert!(
        TEST_API_ROOT
            .scope(
                root,
                LibriVoxMetadataProvider.fetch_snapshot(
                    &MetadataSettings::default(),
                    "6112/recording",
                    "audiobook",
                    false
                )
            )
            .await
            .is_err()
    );
    worker.join().unwrap();
    let (root, worker) = mock_status(vec![("title=How", 503, json!({"error":"unavailable"}))]);
    assert!(
        TEST_API_ROOT
            .scope(root, recording_choices(&local()))
            .await
            .is_err()
    );
    worker.join().unwrap();
    let (root, worker) = mock(vec![("title=How", json!({"error":"No results"}))]);
    assert!(
        TEST_API_ROOT
            .scope(root, recording_choices(&local()))
            .await
            .unwrap()
            .is_empty()
    );
    worker.join().unwrap();
}
