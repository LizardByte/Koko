Always use `cargo +nightly fmt` when formatting rust code.

Always hard pin dependency versions in manifest files.

A pull request may add at most one SQL migration. Combine all schema changes for the pull request into that migration before committing. If superseded development migrations were already applied locally, preserve the database contents while reconciling its schema and migration history.

Keep user documentation strictly focused on what an end user needs to set up, use, or troubleshoot Koko. Do not describe internal algorithms, data structures, implementation decisions, or development history in user guides. Keep Markdown table pipes aligned and order reference lists alphabetically when no meaningful sequence is required.

Begin TypeScript source files with a concise file-level JSDoc description of the module, before imports, exports, or other statements. Keep declaration-level JSDoc immediately above the declaration it describes, and document exported APIs and non-obvious behavior when adding or changing them. Never prepend imports above the file description.

Use Diesel's typed query builder for application data access, including new test fixtures. Do not add raw SQL queries or format SQL strings in Rust. Keep schema changes in SQL migrations. SQLite connection and maintenance commands such as PRAGMA and VACUUM are exceptions when Diesel has no typed equivalent; migration tests may execute the migration SQL directly.
