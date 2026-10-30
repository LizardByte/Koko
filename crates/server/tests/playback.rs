//! Playback quality decisions and FFmpeg argument regressions using typed catalog fixtures.

#[path = "../src/db/schema.rs"]
mod schema;

use diesel::{
    Connection,
    ExpressionMethods,
    RunQueryDsl,
    SqliteConnection,
};
use koko::config::{
    FfmpegSettings,
    MediaLibrarySettings,
};
use koko::media::{
    ClientProfile,
    get_playback_decision,
    list_media_items,
    sync_library_catalog,
};
use koko::transcode::TranscodeSpec;

/// Create a real catalog without depending on installed media inspection tools.
fn catalog() -> (SqliteConnection, i32, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "koko-playback-{}-{}",
        std::process::id(),
        rand::random::<u64>()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("sample.mp4"), b"fixture").unwrap();
    let mut conn = SqliteConnection::establish(":memory:").unwrap();
    koko::db::run_pending_sqlite_migrations(&mut conn).unwrap();
    let library: MediaLibrarySettings = serde_json::from_value(serde_json::json!({
        "name": "Playback", "path": root, "kind": "home_videos", "scanner": "directory"
    }))
    .unwrap();
    let ffmpeg = FfmpegSettings {
        ffmpeg_path: "unused".into(),
        ffprobe_path: "koko-test-no-ffprobe".into(),
    };
    sync_library_catalog(&mut conn, &[library], &ffmpeg).unwrap();
    use schema::media_files as files;
    diesel::update(files::table)
        .set((
            files::container.eq("mp4"),
            files::video_codec.eq("h264"),
            files::audio_codec.eq("aac"),
            files::bit_rate.eq(6_000_000_i64),
            files::width.eq(1920),
            files::height.eq(1080),
        ))
        .execute(&mut conn)
        .unwrap();
    let item = list_media_items(&mut conn, None)
        .unwrap()
        .into_iter()
        .find(|item| item.playable)
        .unwrap();
    (conn, item.id, root)
}

/// Older clients can omit the new explicit conversion preference.
fn profile() -> ClientProfile {
    serde_json::from_value(serde_json::json!({
        "client_type": "web", "client_name": "Playback test", "supported_containers": ["mp4"],
        "supported_video_codecs": ["h264"], "supported_audio_codecs": ["aac"],
        "supported_subtitle_formats": ["vtt"],
        "max_video_width": 0, "max_video_height": 0, "max_bitrate_kbps": 0,
        "supports_adaptive_streaming": false, "prefer_hls": false
    }))
    .unwrap()
}

#[test]
fn quality_limits_reencode_supported_video_instead_of_only_remuxing() {
    let (mut conn, id, root) = catalog();
    let mut client = profile();
    assert!(!client.force_transcode);
    assert!(
        get_playback_decision(&mut conn, id, Some(&client))
            .unwrap()
            .unwrap()
            .can_direct_play
    );
    for (width, height, bitrate) in [(1280, 720, 0), (0, 0, 2000)] {
        client.max_video_width = width;
        client.max_video_height = height;
        client.max_bitrate_kbps = bitrate;
        let decision = get_playback_decision(&mut conn, id, Some(&client))
            .unwrap()
            .unwrap();
        assert!(decision.transcode_required && decision.video_transcode_required);
        assert_eq!(decision.transcode_video_codec.as_deref(), Some("libx264"));
        assert_eq!(decision.transcode_audio_codec.as_deref(), Some("aac"));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn bitrate_boundaries_unknown_rates_and_explicit_recovery_are_honored() {
    let (mut conn, id, root) = catalog();
    let mut client = profile();
    client.max_bitrate_kbps = 6000;
    assert!(
        get_playback_decision(&mut conn, id, Some(&client))
            .unwrap()
            .unwrap()
            .can_direct_play
    );
    diesel::update(schema::media_files::table)
        .set(schema::media_files::bit_rate.eq(None::<i64>))
        .execute(&mut conn)
        .unwrap();
    assert!(
        get_playback_decision(&mut conn, id, Some(&client))
            .unwrap()
            .unwrap()
            .video_transcode_required
    );
    client.max_bitrate_kbps = 0;
    client.force_transcode = true;
    let decision = get_playback_decision(&mut conn, id, Some(&client))
        .unwrap()
        .unwrap();
    assert!(
        decision.video_transcode_required
            && decision.audio_transcode_required
            && !decision.can_direct_play
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn video_targets_reserve_audio_budget_preserve_aspect_ratio_and_allow_overflow_safe_buffers() {
    let mut spec = TranscodeSpec {
        source_path: "sample.mp4".into(),
        audio_only: false,
        output_path: "output.mp4".into(),
        container: "mp4".into(),
        video_codec: Some("libx264".into()),
        audio_codec: Some("aac".into()),
        max_width: Some(854),
        max_height: Some(480),
        max_bitrate_kbps: Some(1500),
        start_time_ms: Some(35000),
        audio_stream_index: Some(1),
    };
    let args = spec.to_ffmpeg_stdout_args();
    assert!(args.windows(2).any(|pair| pair == ["-maxrate", "1308k"]));
    assert!(args.iter().any(|arg| arg.contains("force_divisible_by=2")));
    assert!(args.windows(2).any(|pair| pair == ["-ss", "35.000"]));
    assert!(args.windows(2).any(|pair| pair == ["-map", "0:a:1?"]));
    spec.max_bitrate_kbps = Some(u32::MAX);
    assert!(
        spec.to_ffmpeg_stdout_args()
            .windows(2)
            .any(|pair| pair == ["-bufsize", "8589934206k"])
    );
}
