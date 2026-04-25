pub(crate) mod tmdb;
pub(crate) mod tvdb;

use crate::config::MetadataProviderId;
use crate::metadata::MetadataItemKind;

pub(crate) fn metadata_item_kind(
    provider_id: MetadataProviderId,
    media_type: Option<&str>,
) -> MetadataItemKind {
    match provider_id {
        MetadataProviderId::Tmdb => tmdb::metadata_item_kind(media_type),
        MetadataProviderId::Tvdb => tvdb::metadata_item_kind(media_type),
        _ => MetadataItemKind::Item,
    }
}
