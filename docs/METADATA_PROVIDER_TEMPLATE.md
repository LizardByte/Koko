# Metadata Provider Template

Metadata providers should keep provider-specific API calls, locale mapping, payload parsing, and enrichment inside `crates/server/src/metadata/providers/<provider>.rs`.
`crates/server/src/metadata/mod.rs` should only orchestrate provider calls and persist normalized Koko structures.

## Provider Roles

- `MetadataProviderRole::Primary`: can search and fetch canonical metadata for library items.
- `MetadataProviderRole::Secondary`: extends one or more primary providers with extra metadata, such as theme songs or trailers.

Declare supported library kinds in the descriptor with `supported_kinds`. Secondary providers should also set `extends_provider_ids`.

## Required Steps

1. Add a `MetadataProviderId` variant in `crates/server/src/config.rs`.
2. Create `crates/server/src/metadata/providers/<provider>.rs`.
3. Implement a provider wrapper in `crates/server/src/metadata/providers/mod.rs`.
4. Register the wrapper in `MetadataRegistry::new()`.
5. Return normalized Koko data through `StoredMetadataSnapshot` and `ProviderMetadataDetails`.
6. Add provider-local tests for payload parsing, locale mapping, and edge cases.

## Primary Provider Skeleton

```rust
use crate::config::{MediaLibraryKind, MetadataProviderId, MetadataSettings};
use crate::metadata::{
    MetadataItemKind, MetadataProviderDescriptor, MetadataProviderRole, MetadataSearchResult,
    ProviderMetadataDetails, StoredMetadataSnapshot,
};

pub(crate) fn descriptor() -> MetadataProviderDescriptor {
    MetadataProviderDescriptor {
        id: MetadataProviderId::Example,
        display_name: "Example".into(),
        description: "Primary metadata provider for ...".into(),
        supported_kinds: vec![MediaLibraryKind::Movies, MediaLibraryKind::Shows],
        requires_api_key: true,
        implemented: true,
        role: MetadataProviderRole::Primary,
        extends_provider_ids: Vec::new(),
        attribution_text: "Metadata provided by Example.".into(),
        attribution_url: "https://example.test/".into(),
        logo_light_url: None,
        logo_dark_url: None,
    }
}

pub(crate) fn metadata_item_kind(media_type: Option<&str>) -> MetadataItemKind {
    match media_type.unwrap_or_default().trim() {
        "movie" => MetadataItemKind::Movie,
        "series" => MetadataItemKind::Show,
        _ => MetadataItemKind::Item,
    }
}

pub(crate) async fn search(
    settings: &MetadataSettings,
    query: &str,
    media_type: Option<&str>,
) -> Result<Vec<MetadataSearchResult>, String> {
    // Read this provider's settings, call the provider API, and map results into MetadataSearchResult.
    // Use media_type to choose or filter provider-specific search types.
    todo!()
}

pub(crate) async fn fetch_snapshot(
    settings: &MetadataSettings,
    external_id: &str,
    media_type: &str,
) -> Result<StoredMetadataSnapshot, String> {
    // Fetch provider payload and map core item fields into StoredMetadataSnapshot.
    // Keep raw payload only for diagnostics/re-normalization, not for serving UI data directly.
    todo!()
}

pub(crate) fn metadata_details(snapshot: &StoredMetadataSnapshot) -> ProviderMetadataDetails {
    // Parse the provider payload here and return database-ready extras:
    // external_ids, tagline, logo_url, genres, rating, content_rating, trailers, collections, people.
    ProviderMetadataDetails::default()
}
```

## Secondary Provider Skeleton

```rust
use crate::config::{MediaLibraryKind, MetadataProviderId};
use crate::metadata::{MetadataProviderDescriptor, MetadataProviderRole};

pub(crate) fn descriptor() -> MetadataProviderDescriptor {
    MetadataProviderDescriptor {
        id: MetadataProviderId::ExampleSecondary,
        display_name: "Example Secondary".into(),
        description: "Secondary provider that enriches primary metadata.".into(),
        supported_kinds: vec![MediaLibraryKind::Movies, MediaLibraryKind::Shows],
        requires_api_key: false,
        implemented: true,
        role: MetadataProviderRole::Secondary,
        extends_provider_ids: vec![MetadataProviderId::Tmdb],
        attribution_text: "Extra metadata provided by Example Secondary.".into(),
        attribution_url: "https://example.test/".into(),
        logo_light_url: None,
        logo_dark_url: None,
    }
}
```

## Rules Of Thumb

- Do not add provider URLs, locale maps, payload parsing, or provider ID branches to `metadata/mod.rs`.
- Do not serve UI metadata by reading cached provider JSON. Normalize provider fields into database columns during refresh/upsert.
- Put useful cross-provider IDs in `ProviderMetadataDetails.external_ids`, not in provider JSON-only fallbacks.
- Keep provider payload JSON optional and diagnostic. If a field is useful to Koko, add it to `ProviderMetadataDetails` or a dedicated database structure.
- Provider modules can use shared Koko helpers for caching and storage paths, but they own the provider-specific interpretation of payloads.
