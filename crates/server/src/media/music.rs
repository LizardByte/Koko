//! Local music identities and tag normalization.

mod playback;
mod storage;
use super::*;
pub(super) use playback::album_playback_targets;
pub(super) use storage::{
    apply_detail,
    apply_summaries,
    has_file,
    load_files,
    load_items,
    save_file,
    save_item,
};
pub(crate) use storage::{
    provider_genres,
    save_provider_genres,
};

/// Music tags used for grouping, ordering, and MusicBrainz matching.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct MusicMetadata {
    /// Performing artist credited on this track.
    pub artist: String,
    /// Artist credited for the album, including Various Artists compilations.
    pub album_artist: String,
    /// Album title.
    pub album: String,
    /// Track position within its disc.
    pub track_number: Option<i32>,
    /// Disc position within the album.
    pub disc_number: Option<i32>,
    /// Release year from local tags.
    pub year: Option<i32>,
    /// Locally tagged genres.
    pub genres: Vec<String>,
    /// MusicBrainz album artist identifier from local tags.
    pub musicbrainz_artist_id: Option<String>,
    /// MusicBrainz release identifier from local tags.
    pub musicbrainz_release_id: Option<String>,
    /// MusicBrainz recording identifier from local tags.
    pub musicbrainz_recording_id: Option<String>,
}

/// Keep each artist's albums ahead of its tracks, with tracks in disc order.
pub(crate) fn metadata_sort_key(
    item_type: &str,
    title: &str,
    music: &MusicMetadata,
) -> (String, u8, String, i32, i32, String) {
    (
        music.album_artist.to_lowercase(),
        match item_type {
            "artist" => 0,
            "album" => 1,
            _ => 2,
        },
        music.album.to_lowercase(),
        music.disc_number.unwrap_or(1),
        music.track_number.unwrap_or(i32::MAX),
        title.to_lowercase(),
    )
}

pub(crate) fn valid_mbid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) { byte == b'-' } else { byte.is_ascii_hexdigit() }
        })
}

fn tag_key(value: &str) -> String {
    value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}

fn number(value: Option<String>) -> Option<i32> {
    value?
        .split('/')
        .next()?
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|value| *value > 0)
}

impl MusicMetadata {
    pub(super) fn from_probe(
        raw: Option<&str>,
        relative_path: &str,
    ) -> (Self, String) {
        let payload = raw.and_then(|raw| serde_json::from_str::<Value>(raw).ok());
        let mut tags = HashMap::new();
        if let Some(payload) = &payload {
            let audio = payload
                .get("streams")
                .and_then(Value::as_array)
                .and_then(|streams| {
                    streams
                        .iter()
                        .find(|stream| stream["codec_type"] == "audio")
                });
            for source in [
                audio.and_then(|stream| stream.get("tags")),
                payload.get("format").and_then(|format| format.get("tags")),
            ]
            .into_iter()
            .flatten()
            {
                if let Some(values) = source.as_object() {
                    for (key, value) in values {
                        if let Some(value) = value
                            .as_str()
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                        {
                            tags.insert(tag_key(key), value.to_string());
                        }
                    }
                }
            }
        }
        let tag = |keys: &[&str]| keys.iter().find_map(|key| tags.get(*key).cloned());
        let path = relative_path.replace('\\', "/");
        let mut directories = path.split('/').collect::<Vec<_>>();
        directories.pop();
        let inferred_disc = infer_disc_directory(&directories);
        if inferred_disc.is_some() {
            directories.pop();
        }
        let album = tag(&["album"])
            .or_else(|| directories.last().map(|value| value.to_string()))
            .unwrap_or_else(|| "Unknown Album".into());
        let artist_directory = directories
            .len()
            .checked_sub(2)
            .and_then(|index| directories.get(index));
        let artist = tag(&["artist"])
            .or_else(|| artist_directory.map(|value| value.to_string()))
            .unwrap_or_else(|| "Unknown Artist".into());
        let album_artist = tag(&["albumartist"]).unwrap_or_else(|| {
            if tag(&["compilation"]).as_deref() == Some("1") {
                "Various Artists".into()
            } else {
                artist.clone()
            }
        });
        let fallback = fallback_title_from_relative_path(relative_path);
        let trimmed = fallback.trim_start_matches(|character: char| {
            character.is_ascii_digit() || matches!(character, ' ' | '-' | '.' | '_')
        });
        let title = tag(&["title"]).unwrap_or_else(|| {
            if trimmed.is_empty() { fallback.clone() } else { trimmed.to_string() }
        });
        let inferred_track = fallback
            .split(|character: char| !character.is_ascii_digit())
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|value| *value > 0);
        let mbid = |keys: &[&str]| {
            tag(keys)
                .filter(|value| valid_mbid(value))
                .map(|value| value.to_ascii_lowercase())
        };
        (
            Self {
                artist,
                album_artist,
                album,
                track_number: number(tag(&["track", "tracknumber"])).or(inferred_track),
                disc_number: number(tag(&["disc", "discnumber"])).or(inferred_disc),
                year: tag(&["date", "year", "originaldate"])
                    .and_then(|value| value.get(..4)?.parse::<i32>().ok())
                    .filter(|year| *year > 0),
                genres: tag(&["genre"])
                    .map(|value| {
                        value
                            .split(';')
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                            .map(ToOwned::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
                musicbrainz_artist_id: mbid(&[
                    "musicbrainzalbumartistid",
                    "musicbrainzartistid",
                ]),
                musicbrainz_release_id: mbid(&[
                    "musicbrainzalbumid",
                    "musicbrainzreleaseid",
                ]),
                musicbrainz_recording_id: mbid(&[
                    "musicbrainztrackid",
                    "musicbrainzrecordingid",
                ]),
            },
            title,
        )
    }
}

fn infer_disc_directory(directories: &[&str]) -> Option<i32> {
    let directory = directories.last()?.to_lowercase();
    let separator = |c: char| c.is_whitespace() || matches!(c, '-' | '_');
    let parent = directories
        .len()
        .checked_sub(2)
        .and_then(|index| directories.get(index))
        .map(|name| name.to_lowercase());
    let suffix = parent
        .as_ref()
        .and_then(|parent| directory.strip_prefix(parent))
        .filter(|suffix| suffix.starts_with(separator))
        .map(|suffix| suffix.trim_start_matches(separator));
    let number = [
        Some(directory.as_str()),
        suffix,
    ]
    .into_iter()
    .flatten()
    .find_map(|name| {
        ["disc", "disk", "cd"].into_iter().find_map(|prefix| {
            name.strip_prefix(prefix)?
                .trim_start_matches(separator)
                .parse::<i32>()
                .ok()
                .filter(|number| *number > 0)
        })
    });
    number
}

pub(super) fn plan_music_file(
    file: &CatalogMediaFile,
    library_id: i32,
    stored: Option<&(MusicMetadata, String)>,
) -> Vec<PlannedMediaItem> {
    let (music, title) = stored
        .cloned()
        .unwrap_or_else(|| MusicMetadata::from_probe(None, &file.relative_path));
    // Length-prefixed components make stable keys without serializing metadata.
    let artist_name = music.album_artist.to_lowercase();
    let artist_identity = music
        .musicbrainz_artist_id
        .as_deref()
        .unwrap_or(&artist_name);
    let artist_key = format!(
        "music:artist:{library_id}:{}:{artist_identity}",
        artist_identity.len()
    );
    let album_name = music.album.to_lowercase();
    let album_key = if let Some(id) = &music.musicbrainz_release_id {
        format!("music:release:{library_id}:{id}")
    } else {
        format!(
            "music:album:{}:{artist_key}:{}:{album_name}",
            artist_key.len(),
            album_name.len()
        )
    };
    let mut album_music = music.clone();
    album_music.artist = music.album_artist.clone();
    album_music.track_number = None;
    album_music.disc_number = None;
    album_music.musicbrainz_recording_id = None;
    let mut artist_music = album_music.clone();
    artist_music.album.clear();
    artist_music.musicbrainz_release_id = None;
    artist_music.year = None;
    [
        (
            artist_key.clone(),
            None,
            "artist",
            music.album_artist.clone(),
            artist_music,
        ),
        (
            album_key.clone(),
            Some(artist_key),
            "album",
            music.album.clone(),
            album_music,
        ),
        (
            format!("file:{}", file.library_file_id),
            Some(album_key),
            "track",
            title,
            music,
        ),
    ]
    .into_iter()
    .map(
        |(identity_key, parent_identity_key, item_type, display_title, music)| PlannedMediaItem {
            identity_key,
            parent_identity_key,
            item_type: item_type.into(),
            display_title,
            relative_path: (item_type == "track").then(|| file.relative_path.clone()),
            media_kind: Some("audio".into()),
            season_number: None,
            episode_number: None,
            child_count: 0,
            playable: item_type == "track",
            file_size: (item_type == "track").then_some(file.file_size),
            duration_ms: file.duration_ms,
            modified_at: file.modified_at,
            explicit_id: (item_type == "track").then_some(file.library_file_id),
            missing_since: file.missing_since,
            available_leaf_count: i32::from(file.missing_since.is_none()),
            missing_leaf_count: i32::from(file.missing_since.is_some()),
            music: Some(music),
        },
    )
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn album_prefixed_disc_folders_do_not_become_artists() {
        for disc in ["CD 01", "Disc 2", "disk_03"] {
            let path = format!("The Beach Boys/Pet Sounds/Pet Sounds - {disc}/01 Song.flac");
            let (music, title) = MusicMetadata::from_probe(None, &path);
            assert_eq!(music.artist, "The Beach Boys");
            assert_eq!(music.album, "Pet Sounds");
            assert_eq!(title, "Song");
            assert!(music.disc_number.is_some());
        }
        for album in [
            "Pet Sounds - CD 01",
            "CD Collection",
            "Album Discography",
            "CD 0",
        ] {
            let (music, _) =
                MusicMetadata::from_probe(None, &format!("The Beach Boys/{album}/01 Song.flac"));
            assert_eq!(music.artist, "The Beach Boys");
            assert_eq!(music.album, album);
            assert_eq!(music.disc_number, None);
        }
        let (music, _) = MusicMetadata::from_probe(
            None,
            "The Beach Boys/Pet Sounds/PET SOUNDS_CD_04/01 Song.flac",
        );
        assert_eq!(music.disc_number, Some(4));
    }

    #[test]
    fn embedded_covers_do_not_turn_music_into_video() {
        let raw = serde_json::json!({
            "format": {"format_name": "mp3", "duration": "123.5"},
            "streams": [
                {"codec_type": "video", "codec_name": "mjpeg", "width": 1000,
                 "height": 1000, "disposition": {"attached_pic": 1}},
                {"codec_type": "audio", "codec_name": "mp3"}
            ]
        });
        let metadata = parse_ffprobe_metadata(&raw.to_string()).unwrap();
        assert_eq!(metadata.duration_ms, Some(123500));
        assert_eq!(metadata.audio_codec.as_deref(), Some("mp3"));
        assert!(metadata.video_codec.is_none());
        assert!(metadata.width.is_none());
        assert!(parse_ffprobe_metadata("invalid").is_none());
        assert!(!valid_mbid("../../not-an-id"));
    }
}
