Always use `cargo +nightly fmt` when formatting rust code.

Always hard pin dependency versions in manifest files.

Use Diesel's typed query builder for application data access, including new test fixtures. Do not add raw SQL queries or format SQL strings in Rust. Keep schema changes in SQL migrations. SQLite connection and maintenance commands such as PRAGMA and VACUUM are exceptions when Diesel has no typed equivalent; migration tests may execute the migration SQL directly.
