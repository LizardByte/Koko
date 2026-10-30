//! MusicBrainz artist, release, and recording metadata.

mod artist;
mod client;
mod external_ids;
mod members;
mod recordings;

#[cfg(test)]
use std::time::Duration;

use serde::Deserialize;

use super::{
    MediaLibraryKind,
    MetadataProvider,
    MetadataProviderDescriptor,
    MetadataProviderFuture,
    MetadataProviderId,
    MetadataProviderRole,
    MetadataSearchResult,
    MetadataSettings,
    MusicMatchContext,
    ProviderMetadataDetails,
    ProviderMetadataPerson,
    StoredMetadataSnapshot,
};
use crate::media::{
    MusicMetadata,
    valid_mbid,
};

#[cfg(test)]
tokio::task_local! {
    pub(crate) static TEST_API_ROOT: String;
}
/// MusicBrainz's read-only provider; public lookups do not need credentials.
pub(super) struct MusicBrainzMetadataProvider;

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct Entity {
    id: String,
    title: Option<String>,
    name: Option<String>,
    disambiguation: Option<String>,
    date: Option<String>,
    first_release_date: Option<String>,
    country: Option<String>,
    score: Option<u32>,
    track_count: Option<u32>,
    length: Option<i64>,
    media: Vec<Medium>,
    artist_credit: Vec<ArtistCredit>,
    genres: Vec<Genre>,
    cover_art_archive: Option<CoverArt>,
    release_group: Option<ReleaseGroup>,
    releases: Vec<recordings::Release>,
    #[serde(flatten)]
    artist_details: artist::ArtistDetails,
    relations: Vec<external_ids::Relation>,
    #[serde(rename = "koko-members")]
    members: Vec<members::Member>,
}

#[derive(Debug, Deserialize)]
struct ReleaseGroup {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Medium {
    track_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ArtistCredit {
    name: Option<String>,
    artist: Artist,
    #[serde(default)]
    joinphrase: String,
}

#[derive(Debug, Deserialize)]
struct Artist {
    #[serde(default)]
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct Genre {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CoverArt {
    front: bool,
}

impl Entity {
    fn track_count(&self) -> Option<u32> {
        self.track_count
            .or_else(|| {
                (!self.media.is_empty())
                    .then(|| {
                        self.media
                            .iter()
                            .try_fold(0u32, |total, medium| total.checked_add(medium.track_count?))
                    })
                    .flatten()
            })
            .filter(|count| *count > 0)
    }

    fn title(&self) -> &str {
        self.title.as_deref().or(self.name.as_deref()).unwrap_or("")
    }

    fn artist(&self) -> String {
        self.artist_credit
            .iter()
            .map(|credit| {
                format!(
                    "{}{}",
                    credit.name.as_deref().unwrap_or(&credit.artist.name),
                    credit.joinphrase
                )
            })
            .collect()
    }

    fn year(&self) -> Option<i32> {
        self.date
            .as_deref()
            .or(self.first_release_date.as_deref())?
            .get(..4)?
            .parse()
            .ok()
    }

    fn search_result(
        &self,
        entity: &str,
    ) -> Option<MetadataSearchResult> {
        if !valid_mbid(&self.id) || self.title().trim().is_empty() {
            return None;
        }
        let overview = [
            self.artist(),
            self.disambiguation.clone().unwrap_or_default(),
            self.country.clone().unwrap_or_default(),
        ]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        Some(MetadataSearchResult {
            provider_id: MetadataProviderId::MusicBrainz,
            external_id: self.id.clone(),
            media_type: entity.into(),
            title: self.title().into(),
            overview: (!overview.is_empty()).then_some(overview),
            artwork_url: None,
            backdrop_url: None,
            release_year: self.year(),
            score: self.score.map(|score| f64::from(score) / 100.0),
            track_count: (entity == "release").then(|| self.track_count()).flatten(),
        })
    }
}

fn entity_type(media_type: &str) -> Result<&'static str, String> {
    match media_type {
        "artist" => Ok("artist"),
        "album" | "release" => Ok("release"),
        "release-group" => Ok("release-group"),
        "track" | "recording" => Ok("recording"),
        _ => Err(format!("Unsupported MusicBrainz media type: {media_type}")),
    }
}

async fn search_entities(
    query: &str,
    media_type: &str,
) -> Result<Vec<Entity>, String> {
    let entity = entity_type(media_type)?;
    let payload = client::search(query, entity).await?;
    let key = format!("{entity}s");
    serde_json::from_value(
        payload
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("MusicBrainz response is missing {key}"))?,
    )
    .map_err(|error| format!("Invalid MusicBrainz search response: {error}"))
}

fn quoted(value: &str) -> String {
    // Escape Lucene operators rather than treating a tag as query syntax.
    let mut escaped = String::new();
    for character in value.chars() {
        if "+-!(){}[]^\"~*?:\\/&|".contains(character) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    format!("\"{escaped}\"")
}

fn choose_match(
    entities: &[Entity],
    entity: &str,
    title: &str,
    artist: Option<&str>,
    artist_id: Option<&str>,
) -> Option<MetadataSearchResult> {
    let title = matching_name(title);
    let artist = artist.map(matching_name);
    let matches = entities
        .iter()
        .filter(|candidate| {
            matching_name(candidate.title()) == title
                && candidate.score.unwrap_or_default() >= 95
                && artist_id.map_or_else(
                    || {
                        artist
                            .as_ref()
                            .is_none_or(|artist| matching_name(&candidate.artist()) == *artist)
                    },
                    |id| {
                        candidate
                            .artist_credit
                            .iter()
                            .any(|credit| credit.artist.id == id)
                    },
                )
        })
        .collect::<Vec<_>>();
    // A title alone cannot distinguish artists with the same name or album editions.
    if matches.len() == 1 { matches[0].search_result(entity) } else { None }
}

fn matching_name(value: &str) -> String {
    // File names commonly use ASCII apostrophes where MusicBrainz uses typography.
    value
        .trim()
        .to_lowercase()
        .replace(['‘', '’', 'ʼ', '＇'], "'")
}

fn music_query(
    entity: &str,
    title: &str,
    music: &MusicMetadata,
    context: &MusicMatchContext,
) -> (String, Option<String>) {
    let field = match entity {
        "artist" => "artist",
        "release" => "release",
        "release-group" => "releasegroup",
        _ => "recording",
    };
    let artist_id = if entity == "artist" {
        None
    } else {
        context
            .artist_id
            .as_ref()
            .or(music.musicbrainz_artist_id.as_ref())
            .filter(|id| valid_mbid(id))
            .cloned()
    };
    let mut query = format!("{field}:{}", quoted(title));
    if let Some(id) = &artist_id {
        query.push_str(&format!(" AND arid:{id}"));
    } else if entity != "artist" {
        query.push_str(&format!(" AND artist:{}", quoted(&music.album_artist)));
    }
    if entity == "release" {
        if let Some(year) = music.year {
            query.push_str(&format!(" AND date:{year}"));
        }
    }
    (query, artist_id)
}

impl MetadataProvider for MusicBrainzMetadataProvider {
    fn search_album_recordings<'a>(
        &'a self,
        title: &'a str,
        recording_id: Option<&'a str>,
        context: &'a MusicMatchContext,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        Box::pin(async move {
            let Some(query) = album_recording_query(title, recording_id, context) else {
                return Ok(Vec::new());
            };
            Ok(search_entities(&query, "recording")
                .await?
                .iter()
                .filter_map(|candidate| candidate.search_result("recording"))
                .collect())
        })
    }

    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataProviderDescriptor {
            id: MetadataProviderId::MusicBrainz,
            display_name: "MusicBrainz".into(),
            description: "Artist, album, and track metadata from MusicBrainz, with release \
                          artwork from the Cover Art Archive."
                .into(),
            supported_kinds: vec![MediaLibraryKind::Music],
            requires_api_key: false,
            implemented: true,
            role: MetadataProviderRole::Primary,
            extends_provider_ids: Vec::new(),
            attribution_text: "Music metadata is provided by MusicBrainz. Cover art is provided \
                               by the Cover Art Archive."
                .into(),
            attribution_url: "https://musicbrainz.org/".into(),
            logo_light_url: Some("/assets/providers/musicbrainz.svg".into()),
            logo_dark_url: Some("/assets/providers/musicbrainz.svg".into()),
        }
    }

    fn search<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        query: &'a str,
        media_type: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        self.search_with_artist(settings, query, media_type, None)
    }

    fn search_with_artist<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        query: &'a str,
        media_type: Option<&'a str>,
        artist: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        Box::pin(async move {
            let entity = entity_type(media_type.unwrap_or("release"))?;
            let query = manual_search_query(entity, query, artist);
            Ok(search_entities(&query, entity)
                .await?
                .iter()
                .filter_map(|candidate| candidate.search_result(entity))
                .collect())
        })
    }

    fn guess_music_match<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        item_type: &'a str,
        title: &'a str,
        music: &'a MusicMetadata,
        context: &'a MusicMatchContext,
    ) -> MetadataProviderFuture<'a, Option<MetadataSearchResult>> {
        Box::pin(async move {
            // A folder names an album, rather than a specific pressing or edition.
            let entity = if item_type == "album" && music.musicbrainz_release_id.is_none() {
                "release-group"
            } else {
                entity_type(item_type)?
            };
            if entity == "recording" {
                let Some(query) = album_recording_query(
                    title,
                    music.musicbrainz_recording_id.as_deref(),
                    context,
                ) else {
                    return Ok(None);
                };
                let mut entities = search_entities(&query, "recording").await?;
                narrow_recordings_by_duration(&mut entities, context.duration_ms);
                if let Some(result) = choose_match(&entities, "recording", title, None, None) {
                    return Ok(Some(result));
                }
                return recordings::match_by_position(title, music, context).await;
            }
            let id = match entity {
                "artist" => music.musicbrainz_artist_id.as_deref(),
                "release" => music.musicbrainz_release_id.as_deref(),
                "release-group" => None,
                _ => music.musicbrainz_recording_id.as_deref(),
            };
            if let Some(id) = id.filter(|id| valid_mbid(id)) {
                return Ok(Some(MetadataSearchResult {
                    provider_id: MetadataProviderId::MusicBrainz,
                    external_id: id.into(),
                    media_type: entity.into(),
                    title: title.into(),
                    overview: None,
                    artwork_url: None,
                    backdrop_url: None,
                    release_year: music.year,
                    score: Some(1.0),
                    track_count: None,
                }));
            }
            let artist = match entity {
                "artist" => None,
                "release" | "release-group" => Some(music.album_artist.as_str()),
                _ => Some(music.artist.as_str()),
            };
            if title.starts_with("Unknown ")
                || artist.is_some_and(|artist| artist == "Unknown Artist")
            {
                return Ok(None);
            }
            let (query, artist_id) = music_query(entity, title, music, context);
            if entity == "release-group" {
                if let Some(count) = context.track_count {
                    let (mut query, artist_id) = music_query("release", title, music, context);
                    query.push_str(&format!(" AND tracks:{count}"));
                    let releases = search_entities(&query, "release").await?;
                    return Ok(choose_album_match(
                        &releases,
                        title,
                        artist,
                        artist_id.as_deref(),
                        count,
                    ));
                }
            }
            let entities = search_entities(&query, entity).await?;
            Ok(choose_match(
                &entities,
                entity,
                title,
                artist,
                artist_id.as_deref(),
            ))
        })
    }

    fn fetch_snapshot<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        external_id: &'a str,
        media_type: &'a str,
        include_person_details: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(async move {
            let entity = entity_type(media_type)?;
            if !valid_mbid(external_id) {
                return Err("MusicBrainz identifiers must be UUIDs".into());
            }
            let mut payload = client::fetch(external_id, entity).await?;
            if entity == "artist" {
                members::enrich(&mut payload, include_person_details).await;
            }
            snapshot_from_payload(external_id, entity, payload)
        })
    }

    fn fetch_person_metadata<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        external_id: &'a str,
    ) -> MetadataProviderFuture<'a, Option<ProviderMetadataPerson>> {
        Box::pin(async move {
            if !valid_mbid(external_id) {
                return Err("MusicBrainz identifiers must be UUIDs".into());
            }
            Ok(members::person(&client::person(external_id).await?))
        })
    }

    fn metadata_details(
        &self,
        snapshot: &StoredMetadataSnapshot,
    ) -> ProviderMetadataDetails {
        let payload = snapshot
            .provider_payload_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());
        let entity = payload
            .as_ref()
            .and_then(|raw| serde_json::from_value::<Entity>(raw.clone()).ok());
        ProviderMetadataDetails {
            people: if snapshot.media_type.as_deref() == Some("artist") {
                payload
                    .as_ref()
                    .zip(entity.as_ref())
                    .map(|(payload, entity)| members::credits(payload, &entity.members))
                    .unwrap_or_default()
            } else {
                Vec::new()
            },
            external_ids: entity
                .as_ref()
                .map(|entity| {
                    let mut ids = if snapshot.media_type.as_deref() == Some("artist") {
                        external_ids::artist_ids(&entity.relations)
                    } else {
                        Vec::new()
                    };
                    for credit in &entity.artist_credit {
                        if valid_mbid(&credit.artist.id) {
                            ids.push(crate::metadata::ProviderExternalId {
                                source: "musicbrainz_artist".into(),
                                external_id: credit.artist.id.clone(),
                            });
                        }
                    }
                    if let Some(group) = entity
                        .release_group
                        .as_ref()
                        .filter(|group| valid_mbid(&group.id))
                    {
                        ids.push(crate::metadata::ProviderExternalId {
                            source: "musicbrainz_release_group".into(),
                            external_id: group.id.clone(),
                        });
                    }
                    ids
                })
                .unwrap_or_default(),
            genres: entity
                .map(|entity| entity.genres.into_iter().map(|genre| genre.name).collect())
                .unwrap_or_default(),
            ..ProviderMetadataDetails::default()
        }
    }
}

fn album_recording_query(
    title: &str,
    recording_id: Option<&str>,
    context: &MusicMatchContext,
) -> Option<String> {
    let album = album_query(context)?;
    let recording = if let Some(id) = recording_id.filter(|id| valid_mbid(id)) {
        format!("rid:{id}")
    } else {
        format!("recording:{}", quoted(title.trim()))
    };
    Some(format!("{recording} AND {album}"))
}

fn album_query(context: &MusicMatchContext) -> Option<String> {
    Some(
        if let Some(id) = context.release_id.as_deref().filter(|id| valid_mbid(id)) {
            format!("reid:{id}")
        } else {
            let id = context
                .release_group_id
                .as_deref()
                .filter(|id| valid_mbid(id))?;
            format!("rgid:{id}")
        },
    )
}

fn manual_search_query(
    entity: &str,
    title: &str,
    artist: Option<&str>,
) -> String {
    let artist = artist.map(str::trim).filter(|value| !value.is_empty());
    match (entity, artist) {
        ("release" | "release-group", Some(artist)) => {
            let field = if entity == "release" { "release" } else { "releasegroup" };
            format!(
                "{field}:{} AND artist:{}",
                quoted(title.trim()),
                quoted(artist)
            )
        }
        _ => quoted(title.trim()),
    }
}

fn narrow_recordings_by_duration(
    entities: &mut Vec<Entity>,
    duration: Option<i64>,
) {
    let Some(duration) = duration.filter(|duration| *duration > 0) else {
        return;
    };
    let agrees = |candidate: &Entity| {
        candidate
            .length
            .is_some_and(|length| length > 0 && length.abs_diff(duration) <= 2000)
    };
    if entities.iter().any(agrees) {
        entities.retain(agrees);
    } else {
        entities.retain(|candidate| candidate.length.is_none_or(|length| length <= 0));
    }
}

#[cfg(test)]
mod recording_duration_tests {
    use super::*;

    #[test]
    fn typographic_apostrophes_do_not_reject_the_album_recording() {
        let recording = serde_json::json!({
            "id": "7ca705fd-b07b-4434-8176-5878cc6900dc",
            "title": "Don’t Ya", "score": 100, "length": 181560,
            "artist-credit": [{"artist": {"name": "Brett Eldredge"}}]
        });
        let mut entities: Vec<Entity> =
            serde_json::from_value(serde_json::json!([recording])).unwrap();
        narrow_recordings_by_duration(&mut entities, Some(182233));
        assert_eq!(
            choose_match(&entities, "recording", "Don't Ya", None, None)
                .unwrap()
                .external_id,
            "7ca705fd-b07b-4434-8176-5878cc6900dc"
        );
        for title in [
            "Dont Ya",
            "Don't Ya (live)",
            "Don't Ya!",
        ] {
            assert!(choose_match(&entities, "recording", title, None, None).is_none());
        }
        entities[0].score = Some(94);
        assert!(choose_match(&entities, "recording", "Don't Ya", None, None).is_none());
        entities[0].score = Some(100);
        let mut duplicate = recording;
        duplicate["id"] = serde_json::json!("24cc8311-98fd-423a-bed1-97728f5eabc5");
        duplicate["title"] = serde_json::json!("Don't Ya");
        entities.push(serde_json::from_value(duplicate).unwrap());
        assert!(choose_match(&entities, "recording", "Don't Ya", None, None).is_none());
        narrow_recordings_by_duration(&mut entities, Some(220000));
        assert!(entities.is_empty());
    }

    #[test]
    fn studio_recordings_are_distinguished_from_remixes_without_guessing_ties() {
        let rows = serde_json::json!([
            {"id":"cd83ca98-3d2e-4a83-abd0-92b117cbc080", "title":"Seven Nation Army",
                "score":100, "length":258000},
            {"id":"1f9e42d1-97dd-4226-a44b-4ec30b197c07","title":"Seven Nation Army","score":100},
            {"id":"24cc8311-98fd-423a-bed1-97728f5eabc5", "title":"Seven Nation Army",
                "score":100, "length":231813}
        ]);
        let mut entities: Vec<Entity> = serde_json::from_value(rows.clone()).unwrap();
        narrow_recordings_by_duration(&mut entities, Some(232027));
        assert_eq!(
            choose_match(&entities, "recording", "Seven Nation Army", None, None)
                .unwrap()
                .external_id,
            "24cc8311-98fd-423a-bed1-97728f5eabc5"
        );
        let mut entities: Vec<Entity> = serde_json::from_value(rows).unwrap();
        entities[0].length = Some(232000);
        narrow_recordings_by_duration(&mut entities, Some(232027));
        assert!(choose_match(&entities, "recording", "Seven Nation Army", None, None).is_none());
        narrow_recordings_by_duration(&mut entities, Some(500000));
        assert!(entities.is_empty());
    }
}

fn choose_album_match(
    entities: &[Entity],
    title: &str,
    artist: Option<&str>,
    artist_id: Option<&str>,
    count: u32,
) -> Option<MetadataSearchResult> {
    let matching = entities
        .iter()
        .filter(|entity| {
            entity.track_count() == Some(count)
                && choose_match(
                    std::slice::from_ref(*entity),
                    "release",
                    title,
                    artist,
                    artist_id,
                )
                .is_some()
        })
        .collect::<Vec<_>>();
    if matching.len() == 1 {
        return matching[0].search_result("release");
    }
    // The count cannot distinguish pressings of the same album. Link the common
    // release group instead of choosing an arbitrary edition from search order.
    let first = matching.first()?;
    let group = first
        .release_group
        .as_ref()
        .filter(|group| valid_mbid(&group.id))?;
    if !matching.iter().all(|entity| {
        entity
            .release_group
            .as_ref()
            .is_some_and(|other| other.id == group.id)
    }) {
        return None;
    }
    let mut result = first.search_result("release")?;
    result.external_id = group.id.clone();
    result.media_type = "release-group".into();
    result.track_count = None;
    Some(result)
}

fn snapshot_from_payload(
    external_id: &str,
    entity: &str,
    payload: serde_json::Value,
) -> Result<StoredMetadataSnapshot, String> {
    let parsed: Entity = serde_json::from_value(payload.clone())
        .map_err(|error| format!("Invalid MusicBrainz lookup: {error}"))?;
    if parsed.id != external_id || parsed.title().trim().is_empty() {
        return Err("MusicBrainz returned a mismatched or empty entity".into());
    }
    Ok(StoredMetadataSnapshot {
        provider_id: MetadataProviderId::MusicBrainz,
        external_id: external_id.into(),
        media_type: Some(entity.into()),
        title: Some(parsed.title().into()),
        overview: if entity == "artist" {
            artist::overview(
                parsed.disambiguation.as_deref(),
                parsed.country.as_deref(),
                &parsed.artist_details,
            )
        } else {
            parsed
                .disambiguation
                .clone()
                .filter(|value| !value.is_empty())
        },
        artwork_url: match entity {
            "release-group" => Some(format!(
                "https://coverartarchive.org/release-group/{external_id}/front-500"
            )),
            "release"
                if parsed
                    .cover_art_archive
                    .as_ref()
                    .is_some_and(|cover| cover.front) =>
            {
                Some(format!(
                    "https://coverartarchive.org/release/{external_id}/front-500"
                ))
            }
            "release" => parsed
                .release_group
                .as_ref()
                .filter(|group| valid_mbid(&group.id))
                .map(|group| {
                    format!(
                        "https://coverartarchive.org/release-group/{}/front-500",
                        group.id
                    )
                }),
            _ => None,
        },
        backdrop_url: None,
        release_year: parsed.year(),
        locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.into(),
        provider_locale_key: None,
        provider_payload_json: Some(payload.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const MBID: &str = "f5093c06-23e3-404f-aeaa-40f72885ee3a";

    #[tokio::test]
    async fn recording_matches_require_the_linked_album_even_with_embedded_ids() {
        let music = MusicMetadata {
            artist: "Artist".into(),
            album_artist: "Artist".into(),
            album: "Album".into(),
            musicbrainz_recording_id: Some(MBID.into()),
            ..Default::default()
        };
        let context = MusicMatchContext::default();
        assert!(
            MusicBrainzMetadataProvider
                .guess_music_match(
                    &MetadataSettings::default(),
                    "track",
                    "Song",
                    &music,
                    &context,
                )
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            MusicBrainzMetadataProvider
                .search_album_recordings("Song", None, &context)
                .await
                .unwrap()
                .is_empty()
        );
        let release = "d817f725-66c3-366d-8dd1-b22d49c8a766";
        let context = MusicMatchContext {
            release_id: Some(release.into()),
            release_group_id: Some(MBID.into()),
            ..Default::default()
        };
        assert_eq!(
            album_recording_query("Song", Some(MBID), &context).unwrap(),
            format!("rid:{MBID} AND reid:{release}")
        );
        assert_eq!(
            album_recording_query("Song", None, &context).unwrap(),
            format!("recording:\"Song\" AND reid:{release}")
        );
        let context = MusicMatchContext {
            release_group_id: Some(MBID.into()),
            ..Default::default()
        };
        assert_eq!(
            album_recording_query("Song", None, &context).unwrap(),
            format!("recording:\"Song\" AND rgid:{MBID}")
        );
        assert!(
            album_recording_query(
                "Song",
                None,
                &MusicMatchContext {
                    release_id: Some("invalid".into()),
                    ..Default::default()
                }
            )
            .is_none()
        );
    }

    #[tokio::test]
    async fn manual_album_search_sends_the_edited_artist_as_an_escaped_credit_filter() {
        use std::io::{
            Read,
            Write,
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let root = format!("http://{}/ws/2", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut buffer = [0; 8192];
            let size = stream.read(&mut buffer).unwrap();
            let headers = String::from_utf8_lossy(&buffer[..size]);
            let url = reqwest::Url::parse(&format!(
                "http://localhost{}",
                headers.split_whitespace().nth(1).unwrap()
            ))
            .unwrap();
            assert_eq!(url.path(), "/ws/2/release");
            let query = url
                .query_pairs()
                .find(|(key, _)| key == "query")
                .unwrap()
                .1
                .into_owned();
            assert_eq!(
                query,
                r#"release:"Live\: Deluxe" AND artist:"AC\/DC \& Guests\+""#
            );
            let body = serde_json::json!({"releases": [{
                "id": MBID,
                "title": "Live: Deluxe",
                "track-count": 12,
                "artist-credit": [{"artist": {"name": "AC/DC & Guests+"}}]
            }]})
            .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
                 {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let results = TEST_API_ROOT
            .scope(root, async {
                crate::metadata::search_provider_with_artist(
                    &MetadataSettings::default(),
                    MetadataProviderId::MusicBrainz,
                    " Live: Deluxe ",
                    Some("release"),
                    Some(" AC/DC & Guests+ "),
                )
                .await
                .unwrap()
            })
            .await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].track_count, Some(12));
        assert_eq!(results[0].external_id, MBID);
        server.join().unwrap();
    }

    #[test]
    fn clearing_the_artist_filter_restores_title_only_search_and_other_entities_ignore_it() {
        for artist in [None, Some(""), Some("  ")] {
            assert_eq!(manual_search_query("release", "Album", artist), "\"Album\"");
        }
        assert_eq!(
            manual_search_query("artist", "Artist", Some("Other")),
            "\"Artist\""
        );
    }

    #[tokio::test]
    async fn artist_lookup_requests_url_relationships_and_stores_normalized_identifiers() {
        use diesel::{
            Connection,
            QueryDsl,
            RunQueryDsl,
        };
        use std::io::{
            Read,
            Write,
        };

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let root = format!("http://{}/ws/2", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                let mut chunk = [0; 4096];
                let size = stream.read(&mut chunk).unwrap();
                assert!(size > 0 && headers.len() < 16384);
                headers.extend_from_slice(&chunk[..size]);
            }
            let headers = String::from_utf8(headers).unwrap();
            let url = reqwest::Url::parse(&format!(
                "http://localhost{}",
                headers.split_whitespace().nth(1).unwrap()
            ))
            .unwrap();
            assert_eq!(url.path(), format!("/ws/2/artist/{MBID}"));
            let includes = url
                .query_pairs()
                .find(|(key, _)| key == "inc")
                .unwrap()
                .1
                .into_owned();
            assert!(includes.contains("url-rels") && includes.contains("genres"));
            let body =
                serde_json::json!({"id": MBID, "name": "Musician", "type": "Person", "relations": [
                    {"url": {"resource": "https://www.imdb.com/name/nm0000173/"}},
                    {"url": {"resource": "https://www.wikidata.org/wiki/Q1744"}},
                    {"url": {"resource": "https://www.imdb.com/title/tt12345/"}}
                ]})
                .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
                 {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        TEST_API_ROOT
            .scope(root, async {
                let snapshot = MusicBrainzMetadataProvider
                    .fetch_snapshot(&MetadataSettings::default(), MBID, "artist", false)
                    .await
                    .unwrap();
                let mut conn = diesel::SqliteConnection::establish(":memory:").unwrap();
                crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
                diesel::sql_query(
                    "INSERT INTO media_libraries (id,name,path,kind) VALUES \
                     (1,'Music','music','music')",
                )
                .execute(&mut conn)
                .unwrap();
                diesel::sql_query(
                    "INSERT INTO media_items (id,library_id,identity_key,item_type,display_title) \
                     VALUES (1,1,'artist','artist','Musician')",
                )
                .execute(&mut conn)
                .unwrap();
                crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
                use crate::db::schema::item_metadata_external_ids::dsl as ids;
                let stored = ids::item_metadata_external_ids
                    .select((ids::source, ids::external_id))
                    .order(ids::source)
                    .load::<(String, String)>(&mut conn)
                    .unwrap();
                assert_eq!(
                    stored,
                    [
                        ("imdb".into(), "nm0000173".into()),
                        ("musicbrainz".into(), MBID.into()),
                        ("wikidata".into(), "Q1744".into())
                    ]
                );
            })
            .await;
        server.join().unwrap();
    }

    #[test]
    fn edition_without_cover_uses_its_release_group_artwork() {
        let group = "09f10c6c-e9b4-383c-9744-3df6e3d2770e";
        let mut payload = serde_json::json!({"id": MBID, "title": "Album",
            "cover-art-archive": {"front": false}, "release-group": {"id": group}});
        let snapshot = snapshot_from_payload(MBID, "release", payload.clone()).unwrap();
        assert_eq!(
            snapshot.artwork_url,
            Some(format!(
                "https://coverartarchive.org/release-group/{group}/front-500"
            ))
        );
        payload["cover-art-archive"]["front"] = serde_json::json!(true);
        let snapshot = snapshot_from_payload(MBID, "release", payload.clone()).unwrap();
        assert_eq!(
            snapshot.artwork_url,
            Some(format!(
                "https://coverartarchive.org/release/{MBID}/front-500"
            ))
        );
        payload["cover-art-archive"]["front"] = serde_json::json!(false);
        payload["release-group"]["id"] = serde_json::json!("invalid");
        assert!(
            snapshot_from_payload(MBID, "release", payload)
                .unwrap()
                .artwork_url
                .is_none()
        );
    }

    #[test]
    fn album_matches_use_total_tracks_and_leave_ambiguous_editions_unresolved() {
        let group = "09f10c6c-e9b4-383c-9744-3df6e3d2770e";
        let release = |id: &str, count: u32, group: &str| {
            serde_json::json!({
                "id": id, "title": "Album", "score": 100, "track-count": count,
                "release-group": {"id": group},
                "artist-credit": [{"artist": {"name": "Artist"}}]
            })
        };
        let exact = release(MBID, 12, group);
        let expanded = release("64a283a3-8eb7-4488-a99a-f52ffaaced83", 24, group);
        let entities: Vec<Entity> =
            serde_json::from_value(serde_json::json!([expanded, exact.clone()])).unwrap();
        let matched = choose_album_match(&entities, "Album", Some("Artist"), None, 12).unwrap();
        assert_eq!(matched.external_id, MBID);
        assert_eq!(matched.media_type, "release");
        assert_eq!(matched.track_count, Some(12));
        assert!(choose_album_match(&entities, "Album", Some("Artist"), None, 13).is_none());
        assert!(choose_album_match(&entities, "Album", Some("Wrong Artist"), None, 12).is_none());
        let same_album: Vec<Entity> = serde_json::from_value(serde_json::json!([
            exact.clone(),
            release("64a283a3-8eb7-4488-a99a-f52ffaaced83", 12, group)
        ]))
        .unwrap();
        let matched = choose_album_match(&same_album, "Album", Some("Artist"), None, 12).unwrap();
        assert_eq!(matched.external_id, group);
        assert_eq!(matched.media_type, "release-group");
        let ambiguous: Vec<Entity> = serde_json::from_value(serde_json::json!([
            exact,
            release("64a283a3-8eb7-4488-a99a-f52ffaaced83", 12, MBID)
        ]))
        .unwrap();
        assert!(choose_album_match(&ambiguous, "Album", Some("Artist"), None, 12).is_none());
        let multi_disc: Entity = serde_json::from_value(serde_json::json!({
            "id": MBID, "title": "Album", "media": [{"track-count":12},{"track-count":14}]
        }))
        .unwrap();
        assert_eq!(
            multi_disc.search_result("release").unwrap().track_count,
            Some(26)
        );
        assert_eq!(
            multi_disc
                .search_result("release-group")
                .unwrap()
                .track_count,
            None
        );
    }

    #[test]
    fn lookup_preserves_release_identity_and_artwork() {
        let payload = serde_json::json!({
            "id": MBID, "title": "Album", "date": "2020-05-01",
            "cover-art-archive": {"front": true}, "genres": [{"name": "rock"}]
        });
        let snapshot = snapshot_from_payload(MBID, "release", payload).unwrap();
        assert_eq!(snapshot.release_year, Some(2020));
        assert!(snapshot.artwork_url.unwrap().ends_with("/front-500"));
        assert!(
            snapshot_from_payload(
                "invalid",
                "release",
                serde_json::json!({"id": MBID, "title": "Album"})
            )
            .is_err()
        );
    }

    #[test]
    fn artist_facts_are_stored_in_the_scalar_overview_and_reach_library_cards() {
        use diesel::connection::SimpleConnection;
        use diesel::{
            Connection,
            SqliteConnection,
        };
        let snapshot = snapshot_from_payload(
            MBID,
            "artist",
            serde_json::json!({
                "id": MBID, "name": "Artist", "type": "Person",
                "area": {"name": "United States"}, "begin-area": {"name": "Savannah"},
                "life-span": {"begin": "1973-11-19", "end": null, "ended": false}
            }),
        )
        .unwrap();
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
        conn.batch_execute(
            "INSERT INTO media_libraries (id,name,path,kind,metadata_providers_json)
             VALUES (1,'Music','music','music','[\"musicbrainz\"]');
             INSERT INTO media_items (id,library_id,identity_key,item_type,display_title)
             VALUES (1,1,'artist','artist','Local Artist');",
        )
        .unwrap();
        crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
        let items = crate::media::list_media_items(&mut conn, Some(1)).unwrap();
        assert_eq!(
            items[0].overview.as_deref(),
            Some("United States · Born 19 November 1973 in Savannah")
        );
    }

    #[test]
    fn matching_refuses_ambiguous_titles_and_wrong_artists() {
        let raw = serde_json::json!({
            "id": MBID, "title": "Album", "score": 100,
            "artist-credit": [{"name": "Artist", "artist": {"name": "Artist"}}]
        });
        let entities: Vec<Entity> =
            serde_json::from_value(serde_json::json!([raw.clone()])).unwrap();
        assert!(choose_match(&entities, "release", "Album", Some("Artist"), None).is_some());
        assert!(choose_match(&entities, "release", "Album", Some("Other"), None).is_none());
        let ambiguous: Vec<Entity> =
            serde_json::from_value(serde_json::json!([raw.clone(), raw])).unwrap();
        assert!(choose_match(&ambiguous, "release", "Album", Some("Artist"), None).is_none());
        assert_eq!(quoted("AC/DC: \"Live\""), "\"AC\\/DC\\: \\\"Live\\\"\"");
    }

    #[test]
    fn url_parameters_keep_query_syntax_out_of_paths() {
        let request = client::search_request(
            "release:\"A & B+C%\"",
            "release",
            &musicbrainz_rs::MusicBrainzClient::default(),
        )
        .unwrap();
        let url = reqwest::Url::parse(&request.uri().to_string()).unwrap();
        assert_eq!(url.path(), "/ws/2/release");
        assert!(
            url.query_pairs()
                .any(|(key, value)| key == "query" && value == "release:\"A & B+C%\"")
        );
        assert!(entity_type("movie").is_err());
    }

    #[test]
    fn album_groups_supply_covers_and_matched_parents_constrain_searches() {
        let snapshot = snapshot_from_payload(
            MBID,
            "release-group",
            serde_json::json!({
                "id": MBID, "title": "Album", "first-release-date": "1982",
                "artist-credit": [{"artist": {"id": MBID, "name": "Canonical Artist"}}]
            }),
        )
        .unwrap();
        assert_eq!(snapshot.release_year, Some(1982));
        assert_eq!(
            snapshot.artwork_url.as_deref(),
            Some(format!("https://coverartarchive.org/release-group/{MBID}/front-500").as_str())
        );
        let music = MusicMetadata {
            artist: "Local Artist".into(),
            album_artist: "Local Artist".into(),
            album: "Album".into(),
            ..Default::default()
        };
        let context = MusicMatchContext {
            artist_id: Some(MBID.into()),
            release_group_id: Some(MBID.into()),
            ..Default::default()
        };
        let query = album_recording_query("Song", None, &context).unwrap();
        assert!(!query.contains("arid:"));
        assert!(query.contains(&format!("rgid:{MBID}")));
        assert!(!query.contains("artist:\"Local"));
        let entities: Vec<Entity> = serde_json::from_value(serde_json::json!([{
            "id": MBID, "title": "Song", "score": 100,
            "artist-credit": [{"artist": {"id": MBID, "name": "Canonical Artist"}}]
        }]))
        .unwrap();
        assert!(choose_match(&entities, "recording", "Song", None, None).is_some());
        let (query, _) = music_query("release-group", "Album", &music, &context);
        assert!(query.starts_with("releasegroup:"));
    }

    #[tokio::test]
    async fn requests_identify_koko_and_share_a_rate_limit_across_retries() {
        use std::io::{
            Read,
            Write,
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url =
            reqwest::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let server = std::thread::spawn(move || {
            let mut arrivals = Vec::new();
            for index in 0..3 {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(10)))
                    .unwrap();
                let mut buffer = [0; 4096];
                let size = stream.read(&mut buffer).unwrap();
                let headers = String::from_utf8_lossy(&buffer[..size]).to_ascii_lowercase();
                assert!(headers.contains("user-agent: koko/"));
                assert!(headers.contains("https://github.com/lizardbyte/koko"));
                arrivals.push(std::time::Instant::now());
                let response = if index == 0 {
                    "HTTP/1.1 503 Service Unavailable\r\nRetry-After: 1\r\nContent-Length: \
                     0\r\nConnection: close\r\n\r\n"
                } else {
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
                     2\r\nConnection: close\r\n\r\n{}"
                };
                stream.write_all(response.as_bytes()).unwrap();
            }
            for pair in arrivals.windows(2) {
                assert!(pair[1].duration_since(pair[0]) >= Duration::from_millis(990));
            }
        });
        let (first, second) =
            tokio::join!(client::test_request(url.clone()), client::test_request(url));
        first.unwrap();
        second.unwrap();
        server.join().unwrap();
    }

    #[tokio::test]
    async fn folder_album_matches_and_persists_release_group_cover_without_global_enablement() {
        use diesel::{
            Connection,
            RunQueryDsl,
        };
        use std::io::{
            Read,
            Write,
        };
        let artist_id = "46510d46-ce20-4dde-b9ea-207e474353f4";
        let album_id = "29551412-b9fc-4519-9c65-37f8cf14711e";
        let album = serde_json::json!({
            "id": album_id, "title": "Brett Eldredge", "score": 100,
            "first-release-date": "2017-08-04", "genres": [{"name": "country"}],
            "artist-credit": [{"artist": {"id": artist_id, "name": "Brett Eldredge"}}]
        });
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let api_root = format!("http://{}/ws/2", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            for (index, body) in [
                serde_json::json!({"release-groups": [album.clone()]}),
                album,
            ]
            .into_iter()
            .enumerate()
            {
                let deadline = std::time::Instant::now() + Duration::from_secs(20);
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(connection) => break connection,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && std::time::Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(10))
                        }
                        Err(error) => panic!("MusicBrainz test request did not arrive: {error}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut chunk = [0; 4096];
                    let size = stream.read(&mut chunk).unwrap();
                    assert!(size > 0 && request.len() < 16384);
                    request.extend_from_slice(&chunk[..size]);
                }
                let request = String::from_utf8(request).unwrap();
                let path = request.split_whitespace().nth(1).unwrap();
                let url = reqwest::Url::parse(&format!("http://localhost{path}")).unwrap();
                if index == 0 {
                    assert_eq!(url.path(), "/ws/2/release-group");
                    assert!(url.query_pairs().any(|(key, value)| key == "query"
                        && value
                            == format!("releasegroup:\"Brett Eldredge\" AND arid:{artist_id}")));
                } else {
                    assert_eq!(url.path(), format!("/ws/2/release-group/{album_id}"));
                }
                let body = body.to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
                     {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        TEST_API_ROOT
            .scope(api_root, async {
                let settings = MetadataSettings::default();
                assert!(
                    !settings
                        .providers
                        .iter()
                        .find(|provider| provider.id == MetadataProviderId::MusicBrainz)
                        .unwrap()
                        .enabled
                );
                let music = MusicMetadata {
                    artist: "Brett Eldredge".into(),
                    album_artist: "Brett Eldredge".into(),
                    album: "Brett Eldredge".into(),
                    ..Default::default()
                };
                let context = MusicMatchContext {
                    artist_id: Some(artist_id.into()),
                    ..Default::default()
                };
                let provider = MusicBrainzMetadataProvider;
                let result = provider
                    .guess_music_match(&settings, "album", "Brett Eldredge", &music, &context)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(result.media_type, "release-group");
                let snapshot = provider
                    .fetch_snapshot(&settings, &result.external_id, &result.media_type, false)
                    .await
                    .unwrap();
                let mut conn = diesel::SqliteConnection::establish(":memory:").unwrap();
                crate::db::run_pending_sqlite_migrations(&mut conn).unwrap();
                diesel::sql_query(
                    "INSERT INTO media_libraries (id,name,path,kind) VALUES \
                     (1,'Music','music','music')",
                )
                .execute(&mut conn)
                .unwrap();
                diesel::sql_query(
                    "INSERT INTO media_items (id,library_id,identity_key,item_type,display_title) \
                     VALUES (1,1,'album','album','Brett Eldredge')",
                )
                .execute(&mut conn)
                .unwrap();
                crate::metadata::upsert_item_metadata_snapshot(&mut conn, 1, &snapshot).unwrap();
                let link = crate::metadata::get_primary_item_metadata_link(&mut conn, 1)
                    .unwrap()
                    .unwrap();
                assert_eq!(link.external_id, album_id);
                assert_eq!(link.media_type.as_deref(), Some("release-group"));
                assert_eq!(link.refresh_state, "fresh");
                assert_eq!(
                    link.artwork_url,
                    Some(format!(
                        "https://coverartarchive.org/release-group/{album_id}/front-500"
                    ))
                );
                assert!(link.genres_json.is_none());
            })
            .await;
        server.join().unwrap();
    }
}
