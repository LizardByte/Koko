//! Band membership is a credit, never evidence that a band and a person are the same entity.

use serde::{
    Deserialize,
    Serialize,
};
use serde_json::Value;

use crate::media::valid_mbid;
use crate::metadata::{
    MetadataProviderId,
    ProviderMetadataPerson,
    metadata_response_cache_key,
    read_metadata_response_cache_text,
    write_metadata_response_cache_text,
};

const MEMBERSHIP_ID: &str = "5be4c609-9afa-4ea0-910b-12ffb71e3821";

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct Relationship {
    type_id: String,
    direction: String,
    artist: Option<Value>,
    attributes: Vec<String>,
    begin: Option<String>,
    end: Option<String>,
    ended: Option<bool>,
}

// These values live only in transient provider responses and the response cache.
// Persistence projects them into metadata_people, person external IDs and credits.
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Member {
    artist: Value,
    role: String,
    sort_order: i32,
}

fn members(payload: &Value) -> Vec<Member> {
    if !matches!(
        payload.get("type").and_then(Value::as_str),
        Some("Group" | "Orchestra" | "Choir")
    ) {
        return Vec::new();
    }
    let mut members: Vec<(Member, Vec<Relationship>)> = Vec::new();
    for raw in payload
        .get("relations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Ok(mut relation) = serde_json::from_value::<Relationship>(raw.clone()) else {
            continue;
        };
        if relation.type_id != MEMBERSHIP_ID || relation.direction != "backward" {
            continue;
        }
        let Some(artist) = relation.artist.take() else { continue };
        let Some(id) = artist
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| valid_mbid(id))
        else {
            continue;
        };
        if payload.get("id").and_then(Value::as_str) == Some(id)
            || artist
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind != "Person")
            || artist
                .get("name")
                .and_then(Value::as_str)
                .is_none_or(|name| name.trim().is_empty())
        {
            continue;
        }
        if let Some((_, periods)) = members
            .iter_mut()
            .find(|(member, _)| member.artist.get("id").and_then(Value::as_str) == Some(id))
        {
            periods.push(relation);
        } else {
            let sort_order = i32::try_from(members.len()).unwrap_or(i32::MAX);
            members.push((
                Member {
                    artist,
                    role: String::new(),
                    sort_order,
                },
                vec![relation],
            ));
        }
    }
    members
        .into_iter()
        .map(|(mut member, periods)| {
            member.role = membership_role(&periods);
            member
        })
        .collect()
}

fn membership_role(periods: &[Relationship]) -> String {
    let current = periods.iter().any(|period| {
        period.ended != Some(true) && period.end.as_deref().is_none_or(str::is_empty)
    });
    let mut facts = vec![if current { "Member".into() } else { "Former member".into() }];
    let mut attributes = periods
        .iter()
        .flat_map(|period| period.attributes.iter())
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    attributes.sort_unstable();
    attributes.dedup();
    if !attributes.is_empty() {
        facts.push(attributes.join(", "));
    }
    let dates = periods
        .iter()
        .filter_map(|period| {
            let begin = period.begin.as_deref().filter(|value| !value.is_empty());
            let end = period.end.as_deref().filter(|value| !value.is_empty());
            match (begin, end) {
                (Some(begin), Some(end)) => Some(format!("{begin}–{end}")),
                (Some(begin), None) => Some(format!(
                    "{begin}–{}",
                    if period.ended == Some(true) { "unknown" } else { "present" }
                )),
                (None, Some(end)) => Some(format!("until {end}")),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    if !dates.is_empty() {
        facts.push(dates.join("; "));
    }
    facts.join(" · ")
}

pub(super) async fn enrich(
    payload: &mut Value,
    include_details: bool,
) {
    let mut members = members(payload);
    if members.is_empty() {
        return;
    }
    if include_details {
        for member in &mut members {
            let id = member.artist["id"].as_str().expect("validated member ID");
            match member_details(id).await {
                Ok(details)
                    if details.get("id").and_then(Value::as_str) == Some(id)
                        && person(&details).is_some() =>
                {
                    member.artist = details
                }
                Ok(_) => log::warn!("MusicBrainz returned mismatched member {id}"),
                Err(error) => log::warn!("MusicBrainz member {id} details unavailable: {error}"),
            }
        }
    }
    payload["koko-members"] = serde_json::to_value(members).expect("serializable members");
}

async fn member_details(id: &str) -> Result<Value, String> {
    // Test servers must not read or replace real response caches.
    #[cfg(test)]
    if super::TEST_API_ROOT.try_with(|_| ()).is_ok() {
        return super::client::person(id).await;
    }
    let key = metadata_response_cache_key(&MetadataProviderId::MusicBrainz, "member", &[id]);
    if let Some(cached) =
        read_metadata_response_cache_text(&key).and_then(|raw| serde_json::from_str(&raw).ok())
    {
        return Ok(cached);
    }
    let payload = super::client::person(id).await?;
    if payload.get("id").and_then(Value::as_str) == Some(id) {
        write_metadata_response_cache_text(&key, &payload.to_string());
    }
    Ok(payload)
}

pub(super) fn credits(
    payload: &Value,
    members: &[Member],
) -> Vec<ProviderMetadataPerson> {
    if payload.get("type").and_then(Value::as_str) == Some("Person") {
        return person(payload)
            .map(|mut person| {
                person.role = Some("Artist".into());
                person.department = Some("Artists".into());
                person
            })
            .into_iter()
            .collect();
    }
    people(members)
}

pub(super) fn people(members: &[Member]) -> Vec<ProviderMetadataPerson> {
    members
        .iter()
        .filter_map(|member| {
            let mut person = person(&member.artist)?;
            person.role = Some(member.role.clone());
            person.department = Some("Members".into());
            person.sort_order = member.sort_order;
            Some(person)
        })
        .collect()
}

pub(super) fn person(payload: &Value) -> Option<ProviderMetadataPerson> {
    if payload
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind != "Person")
    {
        return None;
    }
    let entity: super::Entity = serde_json::from_value(payload.clone()).ok()?;
    if !valid_mbid(&entity.id) || entity.title().trim().is_empty() {
        return None;
    }
    let text = |pointer| {
        payload
            .pointer(pointer)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(String::from)
    };
    Some(ProviderMetadataPerson {
        external_id: Some(entity.id.clone()),
        external_ids: super::external_ids::artist_ids(&entity.relations),
        name: entity.title().into(),
        known_for: Vec::new(),
        biography: super::artist::overview(
            entity.disambiguation.as_deref(),
            entity.country.as_deref(),
            &entity.artist_details,
        ),
        gender: text("/gender"),
        birthday: text("/life-span/begin"),
        deathday: text("/life-span/end"),
        birth_place: text("/begin-area/name"),
        role: None,
        department: None,
        character_name: None,
        profile_url: Some(format!("https://musicbrainz.org/artist/{}", entity.id)),
        image_url: None,
        cached_image_path: None,
        sort_order: 0,
    })
}

#[cfg(test)]
mod tests;
