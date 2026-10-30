//! Resolve filename track numbers against positions on the matched album.

use super::{
    Deserialize,
    Entity,
    MetadataSearchResult,
    MusicMatchContext,
    MusicMetadata,
    ReleaseGroup,
    album_query,
    client,
    matching_name,
    valid_mbid,
};

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub(super) struct Release {
    id: String,
    release_group: Option<ReleaseGroup>,
    media: Vec<Medium>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct Medium {
    position: Option<i32>,
    track_offset: Option<i32>,
    #[serde(rename = "track")]
    tracks: Vec<Track>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Track {
    position: Option<i32>,
    number: Option<String>,
    title: String,
    length: Option<i64>,
}

pub(super) async fn match_by_position(
    title: &str,
    music: &MusicMetadata,
    context: &MusicMatchContext,
) -> Result<Option<MetadataSearchResult>, String> {
    let Some(query) = position_query(music, context) else {
        return Ok(None);
    };
    let payload = client::search(&query, "recording").await?;
    let entities: Vec<Entity> = serde_json::from_value(
        payload
            .get("recordings")
            .cloned()
            .ok_or("MusicBrainz response is missing recordings")?,
    )
    .map_err(|error| format!("Invalid MusicBrainz search response: {error}"))?;
    // Unseen results could contain another edition's recording at this position.
    if payload
        .get("count")
        .and_then(serde_json::Value::as_u64)
        .is_some_and(|count| count > entities.len() as u64)
    {
        return Ok(None);
    }
    Ok(choose_position(&entities, title, music, context))
}

fn position_query(
    music: &MusicMetadata,
    context: &MusicMatchContext,
) -> Option<String> {
    let number = music.track_number.filter(|number| *number > 0)?;
    // A conflicting tagged identity needs an explicit rematch, rather than an override.
    if music
        .musicbrainz_recording_id
        .as_deref()
        .is_some_and(valid_mbid)
    {
        return None;
    }
    let mut query = format!("tnum:{number} AND {}", album_query(context)?);
    if let Some(disc) = music.disc_number.filter(|disc| *disc > 0) {
        query.push_str(&format!(" AND position:{disc}"));
    }
    Some(query)
}

fn choose_position(
    entities: &[Entity],
    title: &str,
    music: &MusicMetadata,
    context: &MusicMatchContext,
) -> Option<MetadataSearchResult> {
    let number = music.track_number.filter(|number| *number > 0)?;
    let disc = music.disc_number.filter(|disc| *disc > 0);
    let mut matches = entities.iter().filter(|entity| {
        entity
            .releases
            .iter()
            .filter(|release| matching_album(release, context))
            .flat_map(|release| &release.media)
            .filter(|medium| disc.is_none_or(|disc| medium.position == Some(disc)))
            .any(|medium| {
                medium.tracks.iter().any(|track| {
                    track_position(track, medium) == Some(number)
                        && agrees_with_file(entity, track, title, context.duration_ms)
                })
            })
    });
    let first = matches.next()?;
    // Multiple releases of the same recording are fine; different recordings are ambiguous.
    if matches.any(|other| other.id != first.id) {
        return None;
    }
    first.search_result("recording")
}

fn track_position(
    track: &Track,
    medium: &Medium,
) -> Option<i32> {
    // Search responses use track-offset when the displayed number is nonnumeric
    // (for example A2 on vinyl). Never infer offsets for noncontiguous track lists.
    track
        .position
        .or_else(
            || {
                if medium.tracks.len() == 1 { medium.track_offset?.checked_add(1) } else { None }
            },
        )
        .or_else(|| track.number.as_deref()?.parse().ok())
}

fn matching_album(
    release: &Release,
    context: &MusicMatchContext,
) -> bool {
    if let Some(id) = context.release_id.as_deref().filter(|id| valid_mbid(id)) {
        release.id == id
    } else {
        context
            .release_group_id
            .as_deref()
            .filter(|id| valid_mbid(id))
            .is_some_and(|id| {
                release
                    .release_group
                    .as_ref()
                    .is_some_and(|group| group.id == id)
            })
    }
}

fn agrees_with_file(
    entity: &Entity,
    track: &Track,
    title: &str,
    duration: Option<i64>,
) -> bool {
    let length = track
        .length
        .filter(|length| *length > 0)
        .or(entity.length.filter(|length| *length > 0));
    if let Some((duration, length)) = duration.filter(|duration| *duration > 0).zip(length) {
        // Album + position + duration can identify even a generic or misspelled
        // filename. Allow small mastering/encoder padding differences.
        return duration.abs_diff(length) <= 5000;
    }
    // Without duration, require a close name as independent supporting evidence.
    let title = matching_name(title);
    !title.is_empty()
        && [
            track.title.as_str(),
            entity.title(),
        ]
        .into_iter()
        .any(|name| strsim::normalized_levenshtein(&title, &matching_name(name)) >= 0.85)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECORDING: &str = "7ca705fd-b07b-4434-8176-5878cc6900dc";
    const OTHER: &str = "24cc8311-98fd-423a-bed1-97728f5eabc5";
    const RELEASE: &str = "4d2183e5-42a2-4b1e-8014-407cf8553274";
    const GROUP: &str = "aeb121e8-f977-4351-aaab-19023a4a2d74";

    fn fixture() -> serde_json::Value {
        serde_json::json!({
            "id": RECORDING, "title": "Don’t Ya", "length": 181560, "score": 100,
            "releases": [{"id": RELEASE, "release-group": {"id": GROUP},
                "media": [{"position": 1, "track-offset": 1,
                    "track": [{"number": "2", "title": "Don't Ya", "length": 182173}]}]}]
        })
    }

    fn music() -> MusicMetadata {
        MusicMetadata {
            track_number: Some(2),
            ..Default::default()
        }
    }

    fn context() -> MusicMatchContext {
        MusicMatchContext {
            release_group_id: Some(GROUP.into()),
            duration_ms: Some(182233),
            ..Default::default()
        }
    }

    fn selected(
        rows: Vec<serde_json::Value>,
        title: &str,
        music: &MusicMetadata,
        context: &MusicMatchContext,
    ) -> Option<String> {
        let entities = rows
            .into_iter()
            .map(|row| serde_json::from_value(row).unwrap())
            .collect::<Vec<_>>();
        choose_position(&entities, title, music, context).map(|result| result.external_id)
    }

    #[test]
    fn album_position_and_duration_accept_generic_or_misspelled_filenames() {
        for title in [
            "Track 02", "Dont Ya", "Don't Y", "Don't Ya",
        ] {
            assert_eq!(
                selected(vec![fixture()], title, &music(), &context()).as_deref(),
                Some(RECORDING)
            );
        }
        let mut context = context();
        context.duration_ms = Some(220000);
        assert!(selected(vec![fixture()], "Don't Ya", &music(), &context).is_none());
        context.duration_ms = None;
        assert!(selected(vec![fixture()], "Track 02", &music(), &context).is_none());
        assert_eq!(
            selected(vec![fixture()], "Don't Y", &music(), &context).as_deref(),
            Some(RECORDING)
        );
    }

    #[test]
    fn positions_are_checked_on_the_matched_release_and_disc() {
        let mut row = fixture();
        row["releases"][0]["media"][0]["track-offset"] = serde_json::json!(2);
        row["releases"][0]["media"][0]["track"][0]["number"] = serde_json::json!("3");
        // Search fields can match different releases of the same recording.
        // Track 2 on another release does not prove track 2 on our album.
        let mut another = fixture()["releases"][0].clone();
        another["id"] = serde_json::json!(OTHER);
        another["release-group"]["id"] = serde_json::json!(OTHER);
        row["releases"].as_array_mut().unwrap().push(another);
        assert!(selected(vec![row], "Don't Ya", &music(), &context()).is_none());

        let mut music = music();
        music.disc_number = Some(2);
        assert!(selected(vec![fixture()], "Don't Ya", &music, &context()).is_none());
        let mut row = fixture();
        row["releases"][0]["media"][0]["position"] = serde_json::json!(2);
        assert_eq!(
            selected(vec![row.clone()], "Don't Ya", &music, &context()).as_deref(),
            Some(RECORDING)
        );
        let mut context = context();
        context.release_id = Some(OTHER.into());
        assert!(selected(vec![row], "Don't Ya", &music, &context).is_none());
    }

    #[test]
    fn conflicting_recordings_stay_unmatched_but_repeated_editions_are_safe() {
        assert_eq!(
            selected(vec![fixture(), fixture()], "Track 02", &music(), &context()).as_deref(),
            Some(RECORDING)
        );
        let mut other = fixture();
        other["id"] = serde_json::json!(OTHER);
        other["releases"][0]["media"][0]["position"] = serde_json::json!(2);
        assert!(
            selected(
                vec![fixture(), other.clone()],
                "Track 02",
                &music(),
                &context()
            )
            .is_none()
        );
        let mut music = music();
        music.disc_number = Some(1);
        assert_eq!(
            selected(vec![fixture(), other], "Track 02", &music, &context()).as_deref(),
            Some(RECORDING)
        );
    }

    #[test]
    fn numbered_searches_require_album_and_cannot_override_embedded_identifiers() {
        assert_eq!(
            position_query(&music(), &context()).unwrap(),
            format!("tnum:2 AND rgid:{GROUP}")
        );
        assert!(position_query(&music(), &MusicMatchContext::default()).is_none());
        let mut music = music();
        for number in [None, Some(0), Some(-1)] {
            music.track_number = number;
            assert!(position_query(&music, &context()).is_none());
        }
        music.track_number = Some(2);
        music.disc_number = Some(2);
        let mut context = context();
        context.release_id = Some(RELEASE.into());
        assert_eq!(
            position_query(&music, &context).unwrap(),
            format!("tnum:2 AND reid:{RELEASE} AND position:2")
        );
        music.musicbrainz_recording_id = Some(RECORDING.into());
        assert!(position_query(&music, &context).is_none());
    }

    #[test]
    fn vinyl_numbers_use_offsets_and_missing_lengths_require_title_evidence() {
        let mut row = fixture();
        row["releases"][0]["media"][0]["track"][0]["number"] = serde_json::json!("A2");
        assert_eq!(
            selected(vec![row.clone()], "Track 02", &music(), &context()).as_deref(),
            Some(RECORDING)
        );
        row["releases"][0]["media"][0]["track-offset"] = serde_json::Value::Null;
        assert!(selected(vec![row.clone()], "Track 02", &music(), &context()).is_none());
        row["releases"][0]["media"][0]["track"][0]["position"] = serde_json::json!(2);
        row["releases"][0]["media"][0]["track"][0]["length"] = serde_json::Value::Null;
        row["length"] = serde_json::Value::Null;
        assert!(selected(vec![row.clone()], "Track 02", &music(), &context()).is_none());
        assert_eq!(
            selected(vec![row], "Don't Y", &music(), &context()).as_deref(),
            Some(RECORDING)
        );
    }
}
