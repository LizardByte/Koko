//! Select the capture rendition before FFmpeg opens unrelated variants and subtitle streams.

use std::borrow::Cow;

fn attributes(value: &str) -> Vec<(&str, &str)> {
    static ATTRIBUTE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r#"(?:^|,)\s*([A-Z0-9-]+)=("[^"]*"|[^,]*)"#).unwrap()
    });
    ATTRIBUTE
        .captures_iter(value)
        .map(|c| (c.get(1).unwrap().as_str(), c.get(2).unwrap().as_str()))
        .collect()
}

fn attribute<'a>(
    values: &[(&str, &'a str)],
    name: &str,
) -> Option<&'a str> {
    values
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.trim_matches('"'))
}

/// Keep the highest available AVC rendition and its default audio. Quality changes are handled
/// by Koko's player; capture never needs to probe every bitrate or download unused subtitles.
pub(super) fn playback_playlist(value: &str) -> Cow<'_, str> {
    if !value
        .lines()
        .any(|line| line.starts_with("#EXT-X-STREAM-INF:"))
    {
        return Cow::Borrowed(value);
    }
    let lines = value.lines().collect::<Vec<_>>();
    let selected = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let values = attributes(line.strip_prefix("#EXT-X-STREAM-INF:")?);
            let uri = (index + 1..lines.len()).find(|i| !lines[*i].trim().is_empty())?;
            if lines[uri].starts_with('#') {
                return None;
            }
            let codecs = attribute(&values, "CODECS").unwrap_or("");
            let resolution = attribute(&values, "RESOLUTION").and_then(|r| r.split_once('x'));
            let area = resolution
                .and_then(|(w, h)| Some(w.parse::<u64>().ok()?.saturating_mul(h.parse().ok()?)))
                .unwrap_or(0);
            let video = area > 0
                || codecs.is_empty()
                || codecs.split(',').any(|c| {
                    [
                        "avc", "hvc", "hev", "av01", "vp09",
                    ]
                    .iter()
                    .any(|prefix| c.trim().starts_with(prefix))
                });
            let compatible =
                codecs.is_empty() || codecs.contains("avc1") || codecs.contains("avc3");
            let bandwidth = attribute(&values, "BANDWIDTH")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            Some(((video, compatible, area, bandwidth), index, uri))
        })
        .max_by_key(|(rank, _, _)| *rank);
    let Some((_, index, uri)) = selected else {
        return Cow::Borrowed(value);
    };
    let values = attributes(lines[index].trim_start_matches("#EXT-X-STREAM-INF:"));
    let audio_group = attribute(&values, "AUDIO");
    let audio = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let values = attributes(line.strip_prefix("#EXT-X-MEDIA:")?);
            if attribute(&values, "TYPE") != Some("AUDIO")
                || audio_group.is_none()
                || attribute(&values, "GROUP-ID") != audio_group
            {
                return None;
            }
            Some((
                (
                    attribute(&values, "DEFAULT") == Some("YES"),
                    attribute(&values, "AUTOSELECT") == Some("YES"),
                    usize::MAX - index,
                ),
                index,
            ))
        })
        .max_by_key(|(rank, _)| *rank)
        .map(|(_, index)| index);
    let mut output = String::new();
    for (current, line) in lines.iter().enumerate() {
        if current == index {
            let values = values
                .iter()
                .filter(|(name, _)| *name != "SUBTITLES")
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>();
            output.push_str("#EXT-X-STREAM-INF:");
            output.push_str(&values.join(","));
        } else if current == uri {
            output.push_str(line);
        } else if line.starts_with("#EXT-X-STREAM-INF:")
            || line.starts_with("#EXT-X-I-FRAME-STREAM-INF:")
            || !line.starts_with('#')
        {
            continue;
        } else if let Some(media) = line.strip_prefix("#EXT-X-MEDIA:") {
            let media = attributes(media);
            let captions = attribute(&media, "TYPE") == Some("CLOSED-CAPTIONS")
                && attribute(&media, "GROUP-ID") == attribute(&values, "CLOSED-CAPTIONS");
            let video = attribute(&media, "TYPE") == Some("VIDEO")
                && attribute(&values, "VIDEO").is_some()
                && attribute(&media, "GROUP-ID") == attribute(&values, "VIDEO");
            if Some(current) != audio && !captions && !video {
                continue;
            }
            output.push_str(line);
        } else {
            output.push_str(line);
        }
        output.push('\n');
    }
    Cow::Owned(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_one_compatible_video_and_matching_default_audio_without_subtitles() {
        let master = concat!(
            "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-SESSION-KEY:METHOD=AES-128,URI=\"key\"\n",
            "#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",DEFAULT=NO,URI=\"alternate.m3u8\"\n",
            "#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",DEFAULT=YES,URI=\"audio.m3u8\"\n",
            "#EXT-X-MEDIA:TYPE=VIDEO,GROUP-ID=\"angles\",URI=\"angle.m3u8\"\n",
            "#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"subs\",URI=\"unavailable.m3u8\"\n",
            "#EXT-X-STREAM-INF:BANDWIDTH=1000000,RESOLUTION=640x360,AUDIO=\"a\"\nlow.m3u8\n",
            "#EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080,",
            "CODECS=\"avc1.640028,mp4a.40.2\",AUDIO=\"a\",VIDEO=\"angles\",",
            "SUBTITLES=\"subs\"\nhigh.m3u8\n",
            "#EXT-X-STREAM-INF:BANDWIDTH=10000000,RESOLUTION=3840x2160,",
            "CODECS=\"hvc1.1.6.L153,mp4a.40.2\"\nhevc.m3u8\n"
        );
        let selected = playback_playlist(master);
        assert_eq!(selected.matches("#EXT-X-STREAM-INF:").count(), 1);
        for retained in [
            "high.m3u8",
            "audio.m3u8",
            "angle.m3u8",
            "#EXT-X-SESSION-KEY:",
            "mp4a.40.2",
        ] {
            assert!(selected.contains(retained));
        }
        for removed in [
            "low.m3u8",
            "hevc.m3u8",
            "alternate.m3u8",
            "SUBTITLES",
            "unavailable",
        ] {
            assert!(!selected.contains(removed));
        }
    }

    #[test]
    fn media_playlists_and_incomplete_masters_are_preserved() {
        for value in [
            "#EXTM3U\n#EXTINF:2,\nsegment.ts\n",
            "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\n",
        ] {
            assert_eq!(playback_playlist(value), value);
        }
    }
}
