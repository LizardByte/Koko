# Contributor references

End-user setup, usage, and troubleshooting belong in `docs/`. Keep implementation plans,
development procedures, and architectural notes here instead.

- [Development roadmap](./ROADMAP.md)
- [Issue backlog](./known-issues.md)
- [Metadata provider template](./METADATA_PROVIDER_TEMPLATE.md)

## Database migrations

Use Diesel SQL migrations in `crates/server/sql/migrations` for schema and data changes, rather
than Rust startup repair code. Migration directories use opaque revision prefixes; execution
order is declared in `SQLITE_MIGRATION_ORDER` in `crates/server/src/db/mod.rs`.

Create a migration from the repository root with `cargo new-migration add_media_flags`.
Omit the name to be prompted. The command creates `up.sql` and `down.sql`, registers the revision,
and runs `cargo +nightly fmt`.
