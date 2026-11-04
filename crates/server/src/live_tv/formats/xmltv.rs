//! Read XMLTV one top-level entry at a time, without building a guide-sized XML tree.

use super::{
    Guide,
    Program,
    ProgramMetadata,
    Station,
    child_text,
    http_url,
    xml_time,
};
use quick_xml::{
    Reader,
    Writer,
    events::Event,
};
use std::{
    collections::HashSet,
    io::BufRead,
};

struct XmltvReader {
    entry: Option<Writer<Vec<u8>>>,
    root_header: Vec<u8>,
    depth: usize,
    root_seen: bool,
    root_closed: bool,
    declaration_seen: bool,
    doctype_seen: bool,
    guide: Guide,
}

impl XmltvReader {
    fn write_entry(
        &mut self,
        event: &Event<'_>,
    ) -> Result<(), String> {
        if let Some(writer) = &mut self.entry {
            writer
                .write_event(event.borrow())
                .map_err(|_| "Unable to read XMLTV entry")?;
        }
        Ok(())
    }

    fn open(
        &mut self,
        event: &Event<'_>,
        start: &quick_xml::events::BytesStart<'_>,
        now: Option<i64>,
    ) -> Result<(), String> {
        if self.depth == 0 {
            if self.root_seen || start.name().as_ref() != "tv" {
                return Err("The guide must contain one XMLTV tv root".into());
            }
            // Validate attributes, retaining namespace declarations for each independent entry.
            let mut root = Writer::new(Vec::new());
            root.write_event(Event::Empty(start.clone()))
                .map_err(|_| "Invalid XMLTV root")?;
            validate(&root.into_inner())?;
            let mut root = Writer::new(Vec::new());
            root.write_event(Event::Start(start.clone()))
                .map_err(|_| "Invalid XMLTV root")?;
            self.root_header = root.into_inner();
            self.root_seen = true;
            self.root_closed = matches!(event, Event::Empty(_));
        } else if self.depth == 1 {
            self.entry = Some(Writer::new(self.root_header.clone()));
        }
        self.write_entry(event)?;
        if matches!(event, Event::Start(_)) {
            self.depth += 1;
        } else if self.depth == 1 {
            finish_entry(self.entry.take().unwrap(), &mut self.guide, now)?;
        }
        Ok(())
    }

    fn close(
        &mut self,
        event: &Event<'_>,
        now: Option<i64>,
    ) -> Result<(), String> {
        if self.depth == 0 {
            return Err("The guide is not valid XMLTV".into());
        }
        self.write_entry(event)?;
        self.depth -= 1;
        if self.depth == 1 {
            finish_entry(self.entry.take().unwrap(), &mut self.guide, now)?;
        } else if self.depth == 0 {
            self.root_closed = true;
        }
        Ok(())
    }

    fn outside_entry(
        &self,
        event: &Event<'_>,
    ) -> Result<(), String> {
        match event {
            Event::Text(text)
                if self.depth == 0 && !AsRef::<str>::as_ref(text).trim().is_empty() =>
            {
                return Err("The guide is not valid XMLTV".into());
            }
            Event::GeneralRef(_) | Event::CData(_) if self.depth == 0 => {
                return Err("The guide is not valid XMLTV".into());
            }
            Event::GeneralRef(value) => {
                let name: &str = value;
                quick_xml::escape::unescape(&format!("&{name};"))
                    .map_err(|_| "XMLTV custom entities are not supported")?;
            }
            _ => {}
        }
        Ok(())
    }

    fn accept(
        &mut self,
        event: &Event<'_>,
        now: Option<i64>,
    ) -> Result<(), String> {
        match event {
            Event::Decl(_) => {
                if self.root_seen || self.declaration_seen || self.doctype_seen {
                    return Err("The guide is not valid XMLTV".into());
                }
                self.declaration_seen = true;
            }
            Event::DocType(value) => {
                // External XMLTV DTDs are accepted but never fetched; custom entities are
                // forbidden.
                if self.root_seen || self.doctype_seen || value.contains("<!ENTITY") {
                    return Err("XMLTV entity declarations are not supported".into());
                }
                self.doctype_seen = true;
            }
            Event::Start(start) | Event::Empty(start) => self.open(event, start, now)?,
            Event::End(_) => self.close(event, now)?,
            _ if self.entry.is_some() => self.write_entry(event)?,
            _ => self.outside_entry(event)?,
        }
        Ok(())
    }
}

/// Parse entries independently; retained metadata, rather than the document, determines memory use.
/// The optional time applies the server's normal retention window.
pub(crate) fn read(
    input: impl BufRead,
    now: Option<i64>,
) -> Result<Guide, String> {
    let mut reader = Reader::from_reader(input);
    reader.config_mut().enable_all_checks(true);
    let mut buffer = Vec::new();
    let mut state = XmltvReader {
        entry: None,
        root_header: Vec::new(),
        depth: 0,
        root_seen: false,
        root_closed: false,
        declaration_seen: false,
        doctype_seen: false,
        guide: Guide {
            programs: Vec::new(),
            stations: Vec::new(),
        },
    };
    loop {
        buffer.clear();
        let event = reader
            .read_event_into(&mut buffer)
            .map_err(|_| "The guide is not valid XMLTV")?;
        if matches!(event, Event::Eof) {
            if !state.root_closed || state.depth != 0 {
                return Err("The guide is incomplete or has no XMLTV tv root".into());
            }
            break;
        }
        state.accept(&event, now)?;
    }
    Ok(complete_guide(state.guide, now))
}

fn complete_guide(
    mut guide: Guide,
    now: Option<i64>,
) -> Guide {
    let rows = &mut guide.programs;
    rows.sort_by(|a, b| (&a.guide_key, a.starts_at).cmp(&(&b.guide_key, b.starts_at)));
    // XMLTV stop is optional. Infer it only from the next event on the same station,
    // including a boundary event outside the retained schedule.
    for index in 0..rows.len().saturating_sub(1) {
        if rows[index].ends_at == 0 && rows[index].guide_key == rows[index + 1].guide_key {
            rows[index].ends_at = rows[index + 1].starts_at;
        }
    }
    rows.retain(|p| {
        p.ends_at > p.starts_at
            && now.is_none_or(|time| crate::live_tv::retention::contains(p, time))
    });
    rows.dedup_by(|a, b| a.guide_key == b.guide_key && a.starts_at == b.starts_at);
    // Minimal guides may omit channel declarations. Their station IDs remain mappable.
    let mut keys = guide
        .stations
        .iter()
        .map(|s| s.guide_key.clone())
        .collect::<HashSet<_>>();
    for p in rows {
        if keys.insert(p.guide_key.clone()) {
            guide.stations.push(Station {
                source_id: 0,
                guide_key: p.guide_key.clone(),
                name: p.guide_key.clone(),
                number: String::new(),
                aliases: Vec::new(),
                logo: None,
            });
        }
    }
    guide
}

fn validate(xml: &[u8]) -> Result<roxmltree::Document<'_>, String> {
    let xml = std::str::from_utf8(xml).map_err(|_| "The guide must use UTF-8 text")?;
    roxmltree::Document::parse(xml).map_err(|_| "The guide is not valid XMLTV".into())
}

fn finish_entry(
    mut writer: Writer<Vec<u8>>,
    guide: &mut Guide,
    now: Option<i64>,
) -> Result<(), String> {
    writer
        .write_event(Event::End(quick_xml::events::BytesEnd::new("tv")))
        .map_err(|_| "Unable to read XMLTV entry")?;
    let xml = writer.into_inner();
    let doc = validate(&xml)?;
    let node = doc
        .root_element()
        .first_element_child()
        .ok_or("Invalid XMLTV entry")?;
    match node.tag_name().name() {
        "channel" => {
            if let Some(station) = station(node) {
                guide.stations.push(station);
            }
        }
        "programme" => {
            if let Some(mut program) = program(node) {
                // Keep only lightweight boundary events outside the window, so missing stop
                // times can still be inferred when the feed is unordered.
                if now.is_some_and(|time| !crate::live_tv::retention::contains(&program, time))
                    && program.ends_at != 0
                {
                    program.metadata = ProgramMetadata::default();
                    program.title.clear();
                }
                guide.programs.push(program);
            }
        }
        _ => {}
    }
    Ok(())
}

fn station(node: roxmltree::Node<'_, '_>) -> Option<Station> {
    let key = node.attribute("id")?;
    let aliases = node
        .children()
        .filter(|n| n.has_tag_name("display-name"))
        .filter_map(|n| n.text().map(str::to_owned))
        .collect::<Vec<_>>();
    let number = child_text(node, "lcn")
        .or_else(|| {
            aliases
                .iter()
                .find(|s| {
                    s.chars()
                        .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
                })
                .cloned()
        })
        .unwrap_or_default();
    Some(Station {
        source_id: 0,
        guide_key: key.into(),
        name: aliases.first().cloned().unwrap_or_else(|| key.into()),
        number,
        aliases,
        logo: node
            .children()
            .find(|n| n.has_tag_name("icon"))
            .and_then(|n| n.attribute("src"))
            .filter(|url| http_url(url).is_ok())
            .map(str::to_owned),
    })
}

fn program(node: roxmltree::Node<'_, '_>) -> Option<Program> {
    let (Some(key), Some(start)) = (
        node.attribute("channel"),
        node.attribute("start").and_then(xml_time),
    ) else {
        return None;
    };
    let end = node.attribute("stop").and_then(xml_time);
    let title = child_text(node, "title")?;
    let mut metadata = ProgramMetadata {
        subtitle: child_text(node, "sub-title"),
        description: child_text(node, "desc"),
        ..Default::default()
    };
    for child in node.children() {
        match child.tag_name().name() {
            "category" => {
                if let Some(text) = child.text() {
                    metadata.categories.push(text.to_owned());
                }
            }
            "icon" => {
                metadata.image = child
                    .attribute("src")
                    .filter(|s| http_url(s).is_ok())
                    .map(str::to_owned)
            }
            "rating" => metadata.rating = child_text(child, "value"),
            "date" => {
                metadata.year = child
                    .text()
                    .and_then(|value| value.get(..4))
                    .and_then(|year| year.parse().ok())
            }
            "episode-num" => {
                let episode = child.text().map(str::to_owned);
                if child.attribute("system") == Some("onscreen") {
                    metadata.episode = episode;
                } else if metadata.episode_id.is_none() {
                    metadata.episode_id = episode.map(|id| {
                        format!("{}:{id}", child.attribute("system").unwrap_or("unknown"))
                    });
                }
                if metadata.episode.is_none() && child.attribute("system") == Some("xmltv_ns") {
                    let parts = child
                        .text()
                        .unwrap_or_default()
                        .split('.')
                        .take(2)
                        .map(|part| {
                            part.split('/')
                                .next()
                                .unwrap_or_default()
                                .trim()
                                .parse::<u32>()
                                .ok()
                                .and_then(|number| number.checked_add(1))
                        })
                        .collect::<Vec<_>>();
                    if let [Some(season), Some(episode)] = parts.as_slice() {
                        metadata.episode = Some(format!("S{season}E{episode}"));
                    }
                }
            }
            "new" => metadata.is_new = Some(true),
            "premiere" => {
                metadata.is_premiere = Some(true);
                metadata.is_new = Some(true);
            }
            "live" => metadata.is_live = Some(true),
            "finale" => metadata.is_finale = Some(true),
            "previously-shown" => metadata.is_new = Some(false),
            "credits" => {
                metadata.credits = child
                    .children()
                    .filter_map(|c| c.text())
                    .map(str::to_owned)
                    .collect()
            }
            _ => {}
        }
    }
    Some(Program {
        id: 0,
        source_id: 0,
        guide_key: key.into(),
        starts_at: start,
        ends_at: end.unwrap_or(0),
        title,
        metadata,
    })
}
