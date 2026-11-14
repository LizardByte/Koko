# Contributor references

End-user setup, usage, and troubleshooting belong in `docs/`. Keep implementation plans,
development procedures, and architectural notes here instead.

- [Development roadmap](./ROADMAP.md)
- [Issue backlog](./known-issues.md)
- [Metadata provider template](./METADATA_PROVIDER_TEMPLATE.md)

## Native TV builds

Initialize the native dependencies with `git submodule update --init --recursive`. Cargo prepares
the TV component automatically using the shared Rust code in `packaging/live_tv.rs` and CMake.
Windows builds use the matching verified TSDuck SDK; Unix builds compile the pinned source with
Make. TSDuck's Unix code generators require Python, but Koko's build orchestration runs in Rust.

Package builders can prepare the component separately with:

```sh
cargo run --locked -p xtask -- prepare-live-tv \
  --target x86_64-unknown-linux-gnu --destination target/native-tv
```

Use `--source` for an existing checkout, `--cache` for a reusable build directory, `--revision` to
separate source caches, and `--jobs` to select build concurrency. `--system-cpp-runtime` uses the
package's C++ runtime libraries. Flatpak runs this command with `--offline` before building Koko,
then supplies the prepared directory through `KOKO_TSDUCK_ROOT`.

Run the packaging and command-line regressions with `cargo test -p xtask --locked`.

## Database migrations

Use Diesel SQL migrations in `crates/server/sql/migrations` for schema and data changes, rather
than Rust startup repair code. Migration directories use opaque revision prefixes; execution
order is declared in `SQLITE_MIGRATION_ORDER` in `crates/server/src/db/mod.rs`.

Create a migration from the repository root with `cargo new-migration add_media_flags`.
Omit the name to be prompted. The command creates `up.sql` and `down.sql`, registers the revision,
and runs `cargo +nightly fmt`.
