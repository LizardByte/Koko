//! Album resume selection uses the latest listened track, including completed tracks.
use super::super::*;

pub(in crate::media) fn album_playback_targets(
    conn: &mut SqliteConnection,
    user_id: Option<i32>,
    album: &MediaItem,
) -> Result<ContainerPlaybackTargets, diesel::result::Error> {
    let tracks = playable_container_targets(conn, album)?;
    let ids = tracks.iter().map(|track| track.item.id).collect::<Vec<_>>();
    let progress = playback_progress_by_item_id(conn, user_id, &ids)?;
    let latest = tracks
        .iter()
        .enumerate()
        .filter_map(|(index, track)| {
            progress
                .get(&track.item.id)
                .filter(|row| row.position_ms > 0 || row.completed)
                .map(|row| (index, row))
        })
        .max_by_key(|(_, row)| (row.updated_at, row.id));
    let (index, position, resume) = match latest {
        Some((index, row)) if !row.completed => (index, row.position_ms.max(0), true),
        Some((index, _)) => (index + 1, 0, false),
        None => (0, 0, false),
    };
    let book = album.item_type == "audiobook";
    let primary = tracks.get(index).map(|track| MediaPlaybackTarget {
        item_id: track.item.id,
        start_ms: position,
        label: match (book, latest.is_some()) {
            (true, true) => "Continue audiobook",
            (true, false) => "Listen",
            (false, true) => "Continue album",
            (false, false) => "Play album",
        }
        .into(),
        display_title: track.item.display_title.clone(),
        season_number: None,
        episode_number: None,
        resume,
    });
    Ok(ContainerPlaybackTargets {
        primary,
        restart: book
            .then(|| tracks.first())
            .flatten()
            .map(|track| MediaPlaybackTarget {
                item_id: track.item.id,
                start_ms: 0,
                label: "Restart audiobook".into(),
                display_title: track.item.display_title.clone(),
                season_number: None,
                episode_number: None,
                resume: false,
            }),
    })
}
