pub(crate) mod comicvine;
pub(crate) mod libex;
pub(crate) mod librivox;
use comicvine::ComicVineMetadataProvider;
use libex::LibexMetadataProvider;
use librivox::LibriVoxMetadataProvider;
pub(crate) mod openlibrary;
use openlibrary::OpenLibraryMetadataProvider;
pub(crate) mod audiosilo;
use audiosilo::AudioSiloMetadataProvider;
pub(super) mod musicbrainz;
use musicbrainz::MusicBrainzMetadataProvider;
pub(crate) mod fanart;
use fanart::FanartMetadataProvider;
pub(crate) mod lrclib;
use lrclib::LrclibMetadataProvider;

pub(crate) mod themerr;
pub(crate) mod tmdb;
pub(crate) mod trailerdb;
pub(crate) mod tvdb;

use std::future::Future;
use std::pin::Pin;

use crate::config::{
    MediaLibraryKind,
    MetadataProviderId,
    MetadataSettings,
};
use crate::metadata::{
    MetadataItemKind,
    MetadataProviderDescriptor,
    MetadataProviderRole,
    MetadataSearchResult,
    ProviderDescendantTarget,
    ProviderMetadataCollection,
    ProviderMetadataDetails,
    ProviderMetadataPerson,
    StoredMetadataSnapshot,
    normalize_locale_key,
};

/// Boxed async result returned by metadata provider operations.
pub type MetadataProviderFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;

/// Provider identifiers already matched on a music item's parents.
#[derive(Debug, Default)]
pub struct MusicMatchContext {
    /// Local recording duration, used to distinguish studio, live, and remix recordings.
    pub duration_ms: Option<i64>,
    /// Number of active local track items in an album, across every disc.
    pub track_count: Option<u32>,
    /// Matched MusicBrainz album artist identifier.
    pub artist_id: Option<String>,
    /// Matched MusicBrainz edition identifier, when known.
    pub release_id: Option<String>,
    /// Matched MusicBrainz album identifier across editions.
    pub release_group_id: Option<String>,
}

impl MusicMatchContext {
    /// Whether an album has a valid matched release or release-group identifier.
    pub fn has_album(&self) -> bool {
        self.release_id
            .as_deref()
            .is_some_and(crate::media::valid_mbid)
            || self
                .release_group_id
                .as_deref()
                .is_some_and(crate::media::valid_mbid)
    }
}

/// Provider contract for metadata implementations.
pub trait MetadataProvider {
    /// Return the provider descriptor.
    fn descriptor(&self) -> MetadataProviderDescriptor;

    /// Maximum of this provider's community-rating scale, defaulting to ten.
    fn rating_maximum(&self) -> f64 {
        crate::metadata::ratings::DEFAULT_PROVIDER_MAXIMUM
    }

    /// Map a provider-specific media type to Koko's provider-neutral item kind.
    fn metadata_item_kind(
        &self,
        _media_type: Option<&str>,
    ) -> MetadataItemKind {
        MetadataItemKind::Item
    }

    /// Map a Koko locale key to the provider's locale format.
    fn provider_locale_key(
        &self,
        locale_key: &str,
    ) -> String {
        normalize_locale_key(locale_key)
    }

    /// Whether this provider returns locale-specific metadata that should be stored per locale.
    fn uses_localized_metadata(&self) -> bool {
        false
    }

    /// Search this provider for metadata candidates.
    fn search<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        _query: &'a str,
        _media_type: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        unsupported_provider_operation(self.descriptor().display_name, "search")
    }

    /// Search with an optional artist credit when the provider supports it.
    fn search_with_artist<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        query: &'a str,
        media_type: Option<&'a str>,
        _artist: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        self.search(settings, query, media_type)
    }

    /// Fetch one provider metadata snapshot.
    fn fetch_snapshot<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        _external_id: &'a str,
        _media_type: &'a str,
        _include_person_details: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        unsupported_provider_operation(self.descriptor().display_name, "metadata fetch")
    }

    /// Fetch one season snapshot for a linked show descendant.
    fn fetch_season_snapshot<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        _show_external_id: &'a str,
        _season_number: i32,
        _season_external_id: Option<&'a str>,
        _include_person_details: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        unsupported_provider_operation(self.descriptor().display_name, "season metadata fetch")
    }

    /// Fetch one episode snapshot for a linked show descendant.
    fn fetch_episode_snapshot<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        _show_external_id: &'a str,
        _season_number: i32,
        _episode_number: i32,
        _episode_external_id: Option<&'a str>,
        _include_person_details: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        unsupported_provider_operation(self.descriptor().display_name, "episode metadata fetch")
    }

    /// Fetch provider-side metadata for one person, when the provider supports it.
    fn fetch_person_metadata<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        _external_id: &'a str,
    ) -> MetadataProviderFuture<'a, Option<ProviderMetadataPerson>> {
        Box::pin(async { Ok(None) })
    }

    /// Guess the best provider movie match for one library item.
    fn guess_movie_match<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        _relative_path: &'a str,
        _display_title: &'a str,
    ) -> MetadataProviderFuture<'a, Option<MetadataSearchResult>> {
        Box::pin(async { Ok(None) })
    }

    /// Guess the best provider show match for one show item.
    fn guess_show_match<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        _relative_path: &'a str,
        _display_title: &'a str,
    ) -> MetadataProviderFuture<'a, Option<MetadataSearchResult>> {
        Box::pin(async { Ok(None) })
    }

    /// Match a music item using local tags and embedded MusicBrainz identifiers.
    fn guess_music_match<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        _item_type: &'a str,
        _title: &'a str,
        _music: &'a crate::media::MusicMetadata,
        _context: &'a MusicMatchContext,
    ) -> MetadataProviderFuture<'a, Option<MetadataSearchResult>> {
        Box::pin(async { Ok(None) })
    }

    /// Search recordings only within the item's matched album, optionally by recording ID.
    fn search_album_recordings<'a>(
        &'a self,
        _title: &'a str,
        _recording_id: Option<&'a str>,
        _context: &'a MusicMatchContext,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    /// Load provider-side descendant ids for a linked show.
    fn load_show_descendant_targets<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        _show_external_id: &'a str,
    ) -> MetadataProviderFuture<'a, Vec<ProviderDescendantTarget>> {
        unsupported_provider_operation(self.descriptor().display_name, "show descendant lookup")
    }

    /// Extract database-ready metadata fields from a provider snapshot.
    fn metadata_details(
        &self,
        _snapshot: &StoredMetadataSnapshot,
    ) -> ProviderMetadataDetails {
        ProviderMetadataDetails::default()
    }

    /// Cache provider-specific person artwork references into the snapshot payload.
    fn cache_person_assets<'a>(
        &'a self,
        snapshot: &'a StoredMetadataSnapshot,
        _data_dir: &'a str,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(async move { Ok(snapshot.clone()) })
    }

    /// Return whether this secondary provider should attempt a source metadata lookup candidate.
    fn supports_secondary_metadata_reference(
        &self,
        source_provider_id: &MetadataProviderId,
        item_type: &str,
        database_id: &str,
    ) -> bool {
        self.secondary_metadata_reference_priority(source_provider_id, item_type, database_id)
            .is_some()
    }

    /// Sort priority for supported source metadata lookup candidates.
    fn secondary_metadata_reference_priority(
        &self,
        _source_provider_id: &MetadataProviderId,
        _item_type: &str,
        _database_id: &str,
    ) -> Option<usize> {
        Some(0)
    }

    /// Resolve item-level metadata fields contributed by a secondary provider.
    fn fetch_secondary_metadata<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        _item_type: &'a str,
        _database_id: &'a str,
        _external_id: &'a str,
        _locale_key: &'a str,
    ) -> MetadataProviderFuture<'a, Option<ProviderMetadataDetails>> {
        Box::pin(async { Ok(None) })
    }

    /// Resolve collection-level metadata fields contributed by a secondary provider.
    fn fetch_secondary_collection_metadata<'a>(
        &'a self,
        _item_type: &'a str,
        _database_id: &'a str,
        _external_id: &'a str,
        _locale_key: &'a str,
    ) -> MetadataProviderFuture<'a, Option<ProviderMetadataCollection>> {
        Box::pin(async { Ok(None) })
    }
}

fn unsupported_provider_operation<'a, T>(
    display_name: String,
    operation: &'static str,
) -> MetadataProviderFuture<'a, T> {
    Box::pin(async move { Err(format!("{display_name} {operation} is not implemented.")) })
}

struct TmdbMetadataProvider;
struct TvdbMetadataProvider;
struct ThemerrMetadataProvider;
struct TrailerDbMetadataProvider;
struct LocalNfoMetadataProvider;

impl MetadataProvider for TmdbMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        tmdb::descriptor()
    }

    fn uses_localized_metadata(&self) -> bool {
        true
    }

    fn metadata_item_kind(
        &self,
        media_type: Option<&str>,
    ) -> MetadataItemKind {
        tmdb::metadata_item_kind(media_type)
    }

    fn search<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        query: &'a str,
        media_type: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        Box::pin(tmdb::search(settings, query, media_type))
    }

    fn fetch_snapshot<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        external_id: &'a str,
        media_type: &'a str,
        include_person_details: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(tmdb::fetch_snapshot(
            settings,
            external_id,
            media_type,
            include_person_details,
        ))
    }

    fn fetch_season_snapshot<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        show_external_id: &'a str,
        season_number: i32,
        _season_external_id: Option<&'a str>,
        include_person_details: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(tmdb::fetch_season_snapshot(
            settings,
            show_external_id,
            season_number,
            include_person_details,
        ))
    }

    fn fetch_episode_snapshot<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        show_external_id: &'a str,
        season_number: i32,
        episode_number: i32,
        _episode_external_id: Option<&'a str>,
        include_person_details: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(tmdb::fetch_episode_snapshot(
            settings,
            show_external_id,
            season_number,
            episode_number,
            include_person_details,
        ))
    }

    fn fetch_person_metadata<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        external_id: &'a str,
    ) -> MetadataProviderFuture<'a, Option<ProviderMetadataPerson>> {
        Box::pin(tmdb::fetch_person_metadata(settings, external_id))
    }

    fn guess_movie_match<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        relative_path: &'a str,
        display_title: &'a str,
    ) -> MetadataProviderFuture<'a, Option<MetadataSearchResult>> {
        Box::pin(tmdb::guess_movie_match(
            settings,
            relative_path,
            display_title,
        ))
    }

    fn guess_show_match<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        relative_path: &'a str,
        display_title: &'a str,
    ) -> MetadataProviderFuture<'a, Option<MetadataSearchResult>> {
        Box::pin(tmdb::guess_show_match(
            settings,
            relative_path,
            display_title,
        ))
    }

    fn load_show_descendant_targets<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        show_external_id: &'a str,
    ) -> MetadataProviderFuture<'a, Vec<ProviderDescendantTarget>> {
        Box::pin(tmdb::load_show_descendant_targets(
            settings,
            show_external_id,
        ))
    }

    fn metadata_details(
        &self,
        snapshot: &StoredMetadataSnapshot,
    ) -> ProviderMetadataDetails {
        tmdb::metadata_details(snapshot)
    }

    fn cache_person_assets<'a>(
        &'a self,
        snapshot: &'a StoredMetadataSnapshot,
        data_dir: &'a str,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(tmdb::cache_person_assets(snapshot, data_dir))
    }
}

impl MetadataProvider for TvdbMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        tvdb::descriptor()
    }

    fn uses_localized_metadata(&self) -> bool {
        true
    }

    fn metadata_item_kind(
        &self,
        media_type: Option<&str>,
    ) -> MetadataItemKind {
        tvdb::metadata_item_kind(media_type)
    }

    fn provider_locale_key(
        &self,
        locale_key: &str,
    ) -> String {
        match normalize_locale_key(locale_key).as_str() {
            "en-GB" | "en-US" => "eng",
            "es" | "es-ES" => "spa",
            "fr" | "fr-FR" => "fra",
            "de" | "de-DE" => "deu",
            "it" | "it-IT" => "ita",
            "ja" | "ja-JP" => "jpn",
            "pt" | "pt-BR" => "por",
            _ => "eng",
        }
        .to_string()
    }

    fn search<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        query: &'a str,
        media_type: Option<&'a str>,
    ) -> MetadataProviderFuture<'a, Vec<MetadataSearchResult>> {
        Box::pin(tvdb::search(settings, query, media_type))
    }

    fn fetch_snapshot<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        external_id: &'a str,
        media_type: &'a str,
        include_person_details: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(tvdb::fetch_snapshot(
            settings,
            external_id,
            media_type,
            include_person_details,
        ))
    }

    fn fetch_season_snapshot<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        show_external_id: &'a str,
        season_number: i32,
        season_external_id: Option<&'a str>,
        include_person_details: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(async move {
            let season_external_id = season_external_id.ok_or_else(|| {
                "TheTVDB season refresh is missing a season external id.".to_string()
            })?;
            tvdb::fetch_season_snapshot(
                settings,
                show_external_id,
                season_number,
                season_external_id,
                include_person_details,
            )
            .await
        })
    }

    fn fetch_episode_snapshot<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        show_external_id: &'a str,
        season_number: i32,
        episode_number: i32,
        episode_external_id: Option<&'a str>,
        include_person_details: bool,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(async move {
            let episode_external_id = episode_external_id.ok_or_else(|| {
                "TheTVDB episode refresh is missing an episode external id.".to_string()
            })?;
            tvdb::fetch_episode_snapshot(
                settings,
                show_external_id,
                season_number,
                episode_number,
                episode_external_id,
                include_person_details,
            )
            .await
        })
    }

    fn fetch_person_metadata<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        external_id: &'a str,
    ) -> MetadataProviderFuture<'a, Option<ProviderMetadataPerson>> {
        Box::pin(tvdb::fetch_person_metadata(settings, external_id))
    }

    fn guess_movie_match<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        relative_path: &'a str,
        display_title: &'a str,
    ) -> MetadataProviderFuture<'a, Option<MetadataSearchResult>> {
        Box::pin(tvdb::guess_movie_match(
            settings,
            relative_path,
            display_title,
        ))
    }

    fn guess_show_match<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        relative_path: &'a str,
        display_title: &'a str,
    ) -> MetadataProviderFuture<'a, Option<MetadataSearchResult>> {
        Box::pin(tvdb::guess_show_match(
            settings,
            relative_path,
            display_title,
        ))
    }

    fn load_show_descendant_targets<'a>(
        &'a self,
        settings: &'a MetadataSettings,
        show_external_id: &'a str,
    ) -> MetadataProviderFuture<'a, Vec<ProviderDescendantTarget>> {
        Box::pin(tvdb::load_show_descendant_targets(
            settings,
            show_external_id,
        ))
    }

    fn metadata_details(
        &self,
        snapshot: &StoredMetadataSnapshot,
    ) -> ProviderMetadataDetails {
        tvdb::metadata_details(snapshot)
    }

    fn cache_person_assets<'a>(
        &'a self,
        snapshot: &'a StoredMetadataSnapshot,
        data_dir: &'a str,
    ) -> MetadataProviderFuture<'a, StoredMetadataSnapshot> {
        Box::pin(tvdb::cache_person_assets(snapshot, data_dir))
    }
}

impl MetadataProvider for ThemerrMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        themerr::descriptor()
    }

    fn secondary_metadata_reference_priority(
        &self,
        source_provider_id: &MetadataProviderId,
        item_type: &str,
        database_id: &str,
    ) -> Option<usize> {
        themerr::item_lookup_reference_priority(source_provider_id, item_type, database_id)
    }

    fn fetch_secondary_metadata<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        item_type: &'a str,
        database_id: &'a str,
        external_id: &'a str,
        _locale_key: &'a str,
    ) -> MetadataProviderFuture<'a, Option<ProviderMetadataDetails>> {
        Box::pin(themerr::fetch_youtube_theme_metadata(
            item_type,
            database_id,
            external_id,
        ))
    }

    fn fetch_secondary_collection_metadata<'a>(
        &'a self,
        item_type: &'a str,
        database_id: &'a str,
        external_id: &'a str,
        _locale_key: &'a str,
    ) -> MetadataProviderFuture<'a, Option<ProviderMetadataCollection>> {
        Box::pin(async move {
            Ok(
                themerr::fetch_youtube_theme_url(item_type, database_id, external_id)
                    .await?
                    .map(|theme_song_url| ProviderMetadataCollection {
                        external_id: format!("{item_type}:{database_id}:{external_id}"),
                        name: None,
                        overview: None,
                        artwork_url: None,
                        backdrop_url: None,
                        theme_song_url: Some(theme_song_url),
                    }),
            )
        })
    }
}

impl MetadataProvider for TrailerDbMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        trailerdb::descriptor()
    }

    fn uses_localized_metadata(&self) -> bool {
        true
    }

    fn provider_locale_key(
        &self,
        locale_key: &str,
    ) -> String {
        trailerdb::provider_locale_key(locale_key)
    }

    fn fetch_secondary_metadata<'a>(
        &'a self,
        _settings: &'a MetadataSettings,
        item_type: &'a str,
        database_id: &'a str,
        external_id: &'a str,
        locale_key: &'a str,
    ) -> MetadataProviderFuture<'a, Option<ProviderMetadataDetails>> {
        Box::pin(trailerdb::fetch_secondary_metadata(
            item_type,
            database_id,
            external_id,
            locale_key,
        ))
    }
}

impl MetadataProvider for LocalNfoMetadataProvider {
    fn descriptor(&self) -> MetadataProviderDescriptor {
        MetadataProviderDescriptor {
            id: MetadataProviderId::LocalNfo,
            display_name: "Local NFO".into(),
            description: "Planned sidecar metadata provider for locally curated libraries.".into(),
            supported_kinds: vec![
                MediaLibraryKind::Movies,
                MediaLibraryKind::Shows,
                MediaLibraryKind::Music,
                MediaLibraryKind::Books,
                MediaLibraryKind::HomeVideos,
            ],
            requires_api_key: false,
            implemented: false,
            role: MetadataProviderRole::Primary,
            extends_provider_ids: Vec::new(),
            attribution_text: "Local metadata is provided by files in your library.".into(),
            attribution_url: String::new(),
            logo_light_url: Some("/assets/providers/local-nfo.svg".into()),
            logo_dark_url: Some("/assets/providers/local-nfo.svg".into()),
        }
    }
}

/// Registry of known metadata providers.
pub struct MetadataRegistry {
    providers: Vec<Box<dyn MetadataProvider + Send + Sync>>,
}

impl MetadataRegistry {
    /// Create a new registry containing the built-in providers.
    pub fn new() -> Self {
        Self {
            providers: vec![
                Box::new(TmdbMetadataProvider),
                Box::new(TvdbMetadataProvider),
                Box::new(ThemerrMetadataProvider),
                Box::new(TrailerDbMetadataProvider),
                Box::new(MusicBrainzMetadataProvider),
                Box::new(LibexMetadataProvider),
                Box::new(LibriVoxMetadataProvider),
                Box::new(AudioSiloMetadataProvider),
                Box::new(FanartMetadataProvider),
                Box::new(LrclibMetadataProvider),
                Box::new(OpenLibraryMetadataProvider),
                Box::new(ComicVineMetadataProvider),
                Box::new(LocalNfoMetadataProvider),
            ],
        }
    }

    /// Return a provider by stable id.
    pub fn provider(
        &self,
        provider_id: &MetadataProviderId,
    ) -> Option<&(dyn MetadataProvider + Send + Sync)> {
        self.providers
            .iter()
            .map(Box::as_ref)
            .find(|provider| provider.descriptor().id == *provider_id)
    }

    /// Return all built-in provider descriptors.
    pub fn descriptors(&self) -> Vec<MetadataProviderDescriptor> {
        self.providers
            .iter()
            .map(|provider| provider.descriptor())
            .collect()
    }
}

impl Default for MetadataRegistry {
    fn default() -> Self {
        Self::new()
    }
}
