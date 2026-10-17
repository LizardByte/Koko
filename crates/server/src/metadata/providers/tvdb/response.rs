//! Endpoint models from TheTVDB's v4 OpenAPI contract:
//! https://github.com/thetvdb/v4-api/blob/main/docs/swagger.yml
//!
//! Search records, extended records, and translation responses deliberately have
//! separate decoders. Koko's enrichment is stored beside the provider's `data`.

use std::collections::HashMap;

use serde::{
    Deserialize,
    Serialize,
};
use tvdb4::models::{
    ArtworkBaseRecord,
    ArtworkExtendedRecord,
    Character,
    ContentRating,
    GenreBaseRecord,
    PeopleExtendedRecord,
    Release,
    RemoteId,
    SearchResult,
    SeasonType,
    Trailer,
    Translation,
    TranslationExtended,
};

use crate::config::MetadataProviderId;
use crate::metadata::{
    MetadataSearchResult,
    ProviderExternalId,
    ProviderMetadataDetails,
    ProviderMetadataPerson,
    StoredMetadataSnapshot,
    extract_release_year,
    image_format_preference_rank,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct Response<T> {
    pub data: T,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation: Option<Translation>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub people: HashMap<i64, Person>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct Person {
    pub data: PeopleExtendedRecord,
    pub cached_image_path: Option<String>,
    pub gender_label: Option<String>,
}

/// Fields consumed from `GET /movies/{id}/extended` (OpenAPI 4.7.10).
///
/// The SDK's 4.7.8 movie model incorrectly declares each `companies` category
/// as a single Company; the endpoint returns arrays, including empty arrays.
/// Decode only the documented fields Koko uses so unused provider data cannot
/// prevent an otherwise valid movie from refreshing.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MovieExtendedRecord {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub image: Option<String>,
    pub year: Option<String>,
    #[serde(rename = "first_release")]
    pub first_release: Option<Box<Release>>,
    pub translations: Option<Box<TranslationExtended>>,
    pub artworks: Option<Vec<ArtworkBaseRecord>>,
    pub genres: Option<Vec<GenreBaseRecord>>,
    pub content_ratings: Option<Vec<ContentRating>>,
    pub trailers: Option<Vec<Trailer>>,
    pub remote_ids: Option<Vec<RemoteId>>,
    pub characters: Option<Vec<Character>>,
}

/// Consumed series fields, including season identities for descendant discovery.
/// SDK SeasonBaseRecord also embeds the outdated Companies model.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SeriesExtendedRecord {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub image: Option<String>,
    pub year: Option<String>,
    pub first_aired: Option<String>,
    pub translations: Option<Box<TranslationExtended>>,
    pub artworks: Option<Vec<ArtworkExtendedRecord>>,
    pub genres: Option<Vec<GenreBaseRecord>>,
    pub content_ratings: Option<Vec<ContentRating>>,
    pub trailers: Option<Vec<Trailer>>,
    pub remote_ids: Option<Vec<RemoteId>>,
    pub characters: Option<Vec<Character>>,
    pub default_season_type: Option<i64>,
    pub seasons: Option<Vec<SeasonBaseRecord>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct SeasonBaseRecord {
    pub id: Option<i32>,
    pub number: Option<i64>,
    pub r#type: Option<Box<SeasonType>>,
}

/// Consumed fields from `GET /seasons/{id}/extended`. As with movies,
/// unused company categories must not be decoded through the stale SDK model.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct SeasonExtendedRecord {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub image: Option<String>,
    pub year: Option<String>,
    pub translations: Option<Vec<Translation>>,
    pub artwork: Option<Vec<ArtworkBaseRecord>>,
    pub trailers: Option<Vec<Trailer>>,
    pub episodes: Option<Vec<EpisodeBaseRecord>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct EpisodeBaseRecord {
    pub id: Option<i64>,
    pub number: Option<i32>,
}

/// Episode metadata excludes unused nested season records, which also contain
/// company categories in the provider response.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct EpisodeExtendedRecord {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub image: Option<String>,
    pub year: Option<String>,
    pub aired: Option<String>,
    pub translations: Option<Box<TranslationExtended>>,
    pub content_ratings: Option<Vec<ContentRating>>,
    pub trailers: Option<Vec<Trailer>>,
    pub remote_ids: Option<Vec<RemoteId>>,
    pub characters: Option<Vec<Character>>,
}

pub(super) struct Artwork<'a> {
    kind: Option<i64>,
    image: Option<&'a str>,
    language: Option<&'a str>,
    score: f32,
}

impl<'a> From<&'a ArtworkBaseRecord> for Artwork<'a> {
    fn from(value: &'a ArtworkBaseRecord) -> Self {
        Self {
            kind: value.r#type,
            image: value.image.as_deref(),
            language: value.language.as_deref(),
            score: value.score.unwrap_or(0.0),
        }
    }
}

impl<'a> From<&'a ArtworkExtendedRecord> for Artwork<'a> {
    fn from(value: &'a ArtworkExtendedRecord) -> Self {
        Self {
            kind: value.r#type,
            image: value.image.as_deref(),
            language: value.language.as_deref(),
            score: value.score.unwrap_or(0.0),
        }
    }
}

#[derive(Default)]
pub(super) struct Fields<'a> {
    pub name: Option<&'a str>,
    pub overview: Option<&'a str>,
    pub image: Option<&'a str>,
    pub year: Option<&'a str>,
    pub date: Option<&'a str>,
    pub translations: Option<&'a TranslationExtended>,
    pub season_translations: &'a [Translation],
    pub artworks: Vec<Artwork<'a>>,
    pub genres: &'a [GenreBaseRecord],
    pub ratings: &'a [ContentRating],
    pub trailers: &'a [Trailer],
    pub remote_ids: &'a [RemoteId],
}

pub(super) trait Record: Serialize {
    fn fields(&self) -> Fields<'_>;
    fn characters(&self) -> &[Character] {
        &[]
    }
}

impl Record for MovieExtendedRecord {
    fn fields(&self) -> Fields<'_> {
        Fields {
            name: self.name.as_deref(),
            image: self.image.as_deref(),
            year: self.year.as_deref(),
            date: self
                .first_release
                .as_ref()
                .and_then(|release| release.date.as_deref()),
            translations: self.translations.as_deref(),
            artworks: self
                .artworks
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(Artwork::from)
                .collect(),
            genres: self.genres.as_deref().unwrap_or_default(),
            ratings: self.content_ratings.as_deref().unwrap_or_default(),
            trailers: self.trailers.as_deref().unwrap_or_default(),
            remote_ids: self.remote_ids.as_deref().unwrap_or_default(),
            ..Fields::default()
        }
    }

    fn characters(&self) -> &[Character] {
        self.characters.as_deref().unwrap_or_default()
    }
}

impl Record for SeriesExtendedRecord {
    fn fields(&self) -> Fields<'_> {
        Fields {
            name: self.name.as_deref(),
            overview: self.overview.as_deref(),
            image: self.image.as_deref(),
            year: self.year.as_deref(),
            date: self.first_aired.as_deref(),
            translations: self.translations.as_deref(),
            artworks: self
                .artworks
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(Artwork::from)
                .collect(),
            genres: self.genres.as_deref().unwrap_or_default(),
            ratings: self.content_ratings.as_deref().unwrap_or_default(),
            trailers: self.trailers.as_deref().unwrap_or_default(),
            remote_ids: self.remote_ids.as_deref().unwrap_or_default(),
            ..Fields::default()
        }
    }

    fn characters(&self) -> &[Character] {
        self.characters.as_deref().unwrap_or_default()
    }
}

impl Record for SeasonExtendedRecord {
    fn fields(&self) -> Fields<'_> {
        Fields {
            name: self.name.as_deref(),
            image: self.image.as_deref(),
            year: self.year.as_deref(),
            season_translations: self.translations.as_deref().unwrap_or_default(),
            artworks: self
                .artwork
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(Artwork::from)
                .collect(),
            trailers: self.trailers.as_deref().unwrap_or_default(),
            ..Fields::default()
        }
    }
}

impl Record for EpisodeExtendedRecord {
    fn fields(&self) -> Fields<'_> {
        Fields {
            name: self.name.as_deref(),
            overview: self.overview.as_deref(),
            image: self.image.as_deref(),
            year: self.year.as_deref(),
            date: self.aired.as_deref(),
            translations: self.translations.as_deref(),
            ratings: self.content_ratings.as_deref().unwrap_or_default(),
            trailers: self.trailers.as_deref().unwrap_or_default(),
            remote_ids: self.remote_ids.as_deref().unwrap_or_default(),
            ..Fields::default()
        }
    }

    fn characters(&self) -> &[Character] {
        self.characters.as_deref().unwrap_or_default()
    }
}

pub(super) fn text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn translated<'a>(
    records: &'a [Translation],
    language: &str,
    field: impl Fn(&'a Translation) -> Option<&'a str>,
) -> Option<String> {
    super::tvdb_language_preference(language)
        .iter()
        .find_map(|language| {
            records
                .iter()
                .filter(|record| {
                    record
                        .language
                        .as_deref()
                        .is_some_and(|value| value.eq_ignore_ascii_case(language))
                })
                .find_map(|record| text(field(record)))
        })
}

pub(super) fn search_result(
    item: SearchResult,
    language: &str,
) -> Option<MetadataSearchResult> {
    let media_type = item
        .r#type
        .filter(|kind| matches!(kind.as_str(), "movie" | "series"))?;
    // Search `id`/`objectID` identify index entries (e.g. series-123), not API records.
    let external_id = text(item.tvdb_id.as_deref())?;
    let languages = super::tvdb_language_preference(language);
    let title = item
        .translations
        .as_ref()
        .and_then(|values| {
            languages
                .iter()
                .find_map(|key| text(values.get(key).map(String::as_str)))
        })
        .or_else(|| text(item.name.as_deref()))?;
    let overview = item.overviews.as_ref().and_then(|values| {
        languages
            .iter()
            .find_map(|key| text(values.get(key).map(String::as_str)))
    });
    Some(MetadataSearchResult {
        provider_id: MetadataProviderId::Tvdb,
        external_id,
        media_type,
        title,
        overview,
        artwork_url: text(item.image_url.as_deref()),
        backdrop_url: None,
        release_year: item.year.and_then(|year| year.parse().ok()),
        score: None,
    })
}

fn artwork(
    fields: &Fields<'_>,
    types: &[i64],
    language: Option<&str>,
) -> Option<String> {
    types.iter().find_map(|kind| {
        fields
            .artworks
            .iter()
            .filter(|art| art.kind == Some(*kind) && text(art.image).is_some())
            .max_by(|left, right| {
                let rank = |art: &Artwork<'_>| {
                    (
                        language.is_some_and(|language| {
                            art.language
                                .is_some_and(|value| value.eq_ignore_ascii_case(language))
                        }),
                        art.image
                            .map(image_format_preference_rank)
                            .unwrap_or_default(),
                    )
                };
                rank(left)
                    .cmp(&rank(right))
                    .then_with(|| left.score.total_cmp(&right.score))
            })
            .and_then(|art| text(art.image))
    })
}

pub(super) fn image_url(value: Option<&str>) -> Option<String> {
    text(value).map(|url| {
        if url.starts_with("https://") || url.starts_with("http://") {
            url
        } else {
            format!(
                "https://artworks.thetvdb.com/{}",
                url.trim_start_matches('/')
            )
        }
    })
}

pub(super) fn snapshot<T: Record>(
    response: &Response<T>,
    external_id: &str,
    media_type: &str,
    language: &str,
) -> StoredMetadataSnapshot {
    let fields = response.data.fields();
    let name_records = fields
        .translations
        .and_then(|values| values.name_translations.as_deref())
        .unwrap_or(fields.season_translations);
    let overview_records = fields
        .translations
        .and_then(|values| values.overview_translations.as_deref())
        .unwrap_or(fields.season_translations);
    let title = response
        .translation
        .as_ref()
        .and_then(|value| text(value.name.as_deref()))
        .or_else(|| translated(name_records, language, |value| value.name.as_deref()))
        .or_else(|| text(fields.name));
    let overview = response
        .translation
        .as_ref()
        .and_then(|value| text(value.overview.as_deref()))
        .or_else(|| {
            translated(overview_records, language, |value| {
                value.overview.as_deref()
            })
        })
        .or_else(|| {
            if fields.translations.is_none() && response.translation.is_none() {
                text(fields.overview)
            } else {
                None
            }
        });
    StoredMetadataSnapshot {
        provider_id: MetadataProviderId::Tvdb,
        external_id: external_id.into(),
        media_type: Some(media_type.into()),
        title,
        overview,
        artwork_url: image_url(
            artwork(&fields, &[14, 2, 7], None)
                .as_deref()
                .or(fields.image),
        ),
        backdrop_url: image_url(artwork(&fields, &[15, 3, 8], None).as_deref()),
        release_year: fields
            .year
            .and_then(|year| year.parse().ok())
            .or_else(|| extract_release_year(fields.date.map(str::to_owned))),
        locale_key: crate::metadata::DEFAULT_METADATA_LOCALE.into(),
        provider_locale_key: None,
        provider_payload_json: serde_json::to_string(response).ok(),
    }
}

pub(super) fn external_ids(
    primary_id: Option<String>,
    remote_ids: &[RemoteId],
) -> Vec<ProviderExternalId> {
    let mut ids = Vec::new();
    if let Some(external_id) = primary_id {
        ids.push(ProviderExternalId {
            source: "thetvdb".into(),
            external_id,
        });
    }
    for id in remote_ids {
        let source = id
            .r#type
            .and_then(|kind| {
                super::TVDB_REMOTE_ID_SOURCE_TYPES
                    .iter()
                    .find(|source| source.id == kind)
            })
            .or_else(|| {
                id.source_name.as_deref().and_then(|name| {
                    super::TVDB_REMOTE_ID_SOURCE_TYPES
                        .iter()
                        .find(|source| source.source_name.eq_ignore_ascii_case(name))
                })
            });
        if let (Some(source), Some(external_id)) = (source, text(id.id.as_deref())) {
            let id = ProviderExternalId {
                source: source.source.into(),
                external_id,
            };
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

pub(super) fn person_details(
    person: &Person,
    language: &str,
) -> Option<ProviderMetadataPerson> {
    let data = &person.data;
    let external_id = data.id.map(|id| id.to_string());
    let preferred = super::tvdb_requested_language_preference(language);
    let biography = preferred
        .iter()
        .find_map(|language| {
            data.biographies
                .as_deref()
                .unwrap_or_default()
                .iter()
                .filter(|record| {
                    record
                        .language
                        .as_deref()
                        .is_some_and(|value| value.eq_ignore_ascii_case(language))
                })
                .find_map(|record| text(record.biography.as_deref()))
        })
        .or_else(|| {
            data.translations
                .as_deref()
                .and_then(|values| values.overview_translations.as_deref())
                .and_then(|records| {
                    preferred.iter().find_map(|language| {
                        records
                            .iter()
                            .filter(|record| {
                                record
                                    .language
                                    .as_deref()
                                    .is_some_and(|value| value.eq_ignore_ascii_case(language))
                            })
                            .find_map(|record| text(record.overview.as_deref()))
                    })
                })
        });
    Some(ProviderMetadataPerson {
        external_ids: external_ids(
            external_id.clone(),
            data.remote_ids.as_deref().unwrap_or_default(),
        ),
        external_id: external_id.clone(),
        name: text(data.name.as_deref())?,
        known_for: Vec::new(),
        biography,
        gender: person.gender_label.clone(),
        birthday: text(data.birth.as_deref()),
        deathday: text(data.death.as_deref()),
        birth_place: text(data.birth_place.as_deref()),
        role: None,
        department: None,
        character_name: None,
        profile_url: external_id.map(|id| format!("https://thetvdb.com/people/{id}")),
        image_url: image_url(data.image.as_deref()),
        cached_image_path: person.cached_image_path.clone(),
        sort_order: 0,
    })
}

pub(super) fn details<T: Record>(
    response: &Response<T>,
    snapshot: &StoredMetadataSnapshot,
) -> ProviderMetadataDetails {
    let fields = response.data.fields();
    let language = snapshot.provider_locale_key.as_deref().unwrap_or("eng");
    let trailer = fields
        .trailers
        .iter()
        .filter(|trailer| {
            trailer
                .url
                .as_deref()
                .and_then(crate::metadata::youtube_watch_url)
                .is_some()
        })
        .find(|trailer| {
            trailer
                .language
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(language))
        })
        .or_else(|| {
            fields.trailers.iter().find(|trailer| {
                trailer
                    .url
                    .as_deref()
                    .and_then(crate::metadata::youtube_watch_url)
                    .is_some()
            })
        });
    let people = response
        .data
        .characters()
        .iter()
        .enumerate()
        .filter_map(|(index, character)| {
            let id = character.people_id?;
            let enriched = response.people.get(&i64::from(id));
            let shallow = Person {
                data: PeopleExtendedRecord {
                    id: Some(i64::from(id)),
                    name: character.person_name.clone(),
                    image: character.person_img_url.clone(),
                    ..PeopleExtendedRecord::new()
                },
                cached_image_path: None,
                gender_label: None,
            };
            let mut person = person_details(enriched.unwrap_or(&shallow), language)?;
            person.role = text(character.people_type.as_deref());
            person.department = person.role.as_deref().map(|role| {
                if role.eq_ignore_ascii_case("Actor") { "Cast" } else { "Crew" }.into()
            });
            person.character_name =
                text(character.name.as_deref()).filter(|name| name != &person.name);
            person.sort_order = character
                .sort
                .and_then(|order| i32::try_from(order).ok())
                .unwrap_or(index as i32);
            Some(person)
        })
        .collect();
    ProviderMetadataDetails {
        external_ids: external_ids(Some(snapshot.external_id.clone()), fields.remote_ids),
        tagline: response
            .translation
            .as_ref()
            .and_then(|translation| text(translation.tagline.as_deref())),
        logo_url: image_url(artwork(&fields, &[25, 23], Some(language)).as_deref()),
        genres: fields
            .genres
            .iter()
            .filter_map(|genre| text(genre.name.as_deref()))
            .collect(),
        // TheTVDB's score is relative popularity, not a user rating.
        rating: None,
        content_rating: fields
            .ratings
            .iter()
            .find(|rating| {
                rating
                    .country
                    .as_deref()
                    .is_some_and(|country| country.eq_ignore_ascii_case("usa"))
            })
            .or_else(|| fields.ratings.first())
            .and_then(|rating| text(rating.name.as_deref())),
        trailer_title: trailer.and_then(|trailer| text(trailer.name.as_deref())),
        trailer_url: trailer
            .and_then(|trailer| trailer.url.as_deref())
            .and_then(crate::metadata::youtube_watch_url),
        people,
        ..ProviderMetadataDetails::default()
    }
}
