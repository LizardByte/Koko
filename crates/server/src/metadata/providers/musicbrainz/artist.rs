//! Concise artist descriptions from MusicBrainz's structured artist fields.
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub(super) struct ArtistDetails {
    #[serde(rename = "type")]
    kind: Option<String>,
    area: Option<Area>,
    begin_area: Option<Area>,
    end_area: Option<Area>,
    life_span: Option<LifeSpan>,
}

#[derive(Debug, Deserialize)]
struct Area {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LifeSpan {
    begin: Option<String>,
    end: Option<String>,
    ended: Option<bool>,
}

fn text(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn area_name(area: Option<&Area>) -> Option<&str> {
    text(area.and_then(|area| area.name.as_deref()))
}

pub(super) fn overview(
    disambiguation: Option<&str>,
    country: Option<&str>,
    details: &ArtistDetails,
) -> Option<String> {
    let mut facts = Vec::new();
    if let Some(description) = text(disambiguation) {
        facts.push(description.to_string());
    } else if let Some(kind) =
        text(details.kind.as_deref()).filter(|kind| !matches!(*kind, "Person" | "Other"))
    {
        facts.push(kind.to_string());
    }
    if let Some(area) = area_name(details.area.as_ref()).or_else(|| text(country)) {
        facts.push(area.to_string());
    }
    let labels = match details.kind.as_deref() {
        Some("Person") => Some(("Born", "Died")),
        Some("Group" | "Orchestra" | "Choir") => Some(("Formed", "Dissolved")),
        _ => None,
    };
    if let Some((begin_label, end_label)) = labels {
        let life = details.life_span.as_ref();
        if let Some(begin) = event(
            begin_label,
            life.and_then(|life| life.begin.as_deref()),
            details.begin_area.as_ref(),
        ) {
            facts.push(begin);
        }
        if life.is_some_and(|life| life.ended == Some(true) || text(life.end.as_deref()).is_some())
        {
            if let Some(end) = event(
                end_label,
                life.and_then(|life| life.end.as_deref()),
                details.end_area.as_ref(),
            ) {
                facts.push(end);
            }
        }
    }
    (!facts.is_empty()).then(|| facts.join(" · "))
}

fn event(
    label: &str,
    date: Option<&str>,
    area: Option<&Area>,
) -> Option<String> {
    let date = date.and_then(format_date);
    let area = area_name(area);
    match (date, area) {
        (Some(date), Some(area)) => Some(format!("{label} {date} in {area}")),
        (Some(date), None) => Some(format!("{label} {date}")),
        (None, Some(area)) => Some(format!("{label} in {area}")),
        (None, None) => None,
    }
}

fn format_date(value: &str) -> Option<String> {
    let parts = value.split('-').collect::<Vec<_>>();
    let year = parts
        .first()?
        .parse::<i32>()
        .ok()
        .filter(|year| *year > 0)?;
    match parts.as_slice() {
        [_] => Some(format!("{year:04}")),
        [_, month] => chrono::NaiveDate::from_ymd_opt(year, month.parse().ok()?, 1)
            .map(|date| date.format("%B %Y").to_string()),
        [_, month, day] => {
            chrono::NaiveDate::from_ymd_opt(year, month.parse().ok()?, day.parse().ok()?)
                .map(|date| date.format("%-d %B %Y").to_string())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persons_have_birth_dates_instead_of_career_start_dates() {
        let details = serde_json::from_value(serde_json::json!({
            "type": "Person", "area": {"name": "United States"},
            "begin-area": {"name": "Savannah"},
            "life-span": {"begin": "1973-11-19", "end": null, "ended": false}
        }))
        .unwrap();
        assert_eq!(
            overview(None, Some("US"), &details).as_deref(),
            Some("United States · Born 19 November 1973 in Savannah")
        );
    }

    #[test]
    fn group_dates_describe_formation_and_dissolution() {
        let details = serde_json::from_value(serde_json::json!({
            "type": "Group", "area": {"name": "United States"},
            "begin-area": {"name": "Aberdeen"},
            "life-span": {"begin": "1988-01", "end": "1994-04-05", "ended": true}
        }))
        .unwrap();
        assert_eq!(
            overview(Some("90s grunge band"), Some("US"), &details).as_deref(),
            Some(
                "90s grunge band · United States · Formed January 1988 in Aberdeen · Dissolved 5 \
                 April 1994"
            )
        );
        assert_eq!(format_date("1988").as_deref(), Some("1988"));
        assert!(format_date("1988-13").is_none());
        assert!(format_date("1988-02-31").is_none());
    }

    #[test]
    fn missing_facts_and_undefined_life_spans_do_not_invent_descriptions() {
        assert!(overview(None, None, &ArtistDetails::default()).is_none());
        let details = serde_json::from_value(serde_json::json!({
            "type": "Character", "life-span": {"begin": "1990", "end": null}
        }))
        .unwrap();
        assert_eq!(
            overview(Some(" Fictional singer "), None, &details).as_deref(),
            Some("Fictional singer")
        );
    }
}
