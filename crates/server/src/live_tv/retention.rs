//! Bounded guide retention, independent of device availability or successful refreshes.
use crate::db::schema::live_tv_programs as p;
use diesel::prelude::*;

pub(super) const HISTORY: i64 = 86400;
pub(super) const FUTURE: i64 = 21 * 86400;

pub(super) fn contains(
    program: &super::Program,
    now: i64,
) -> bool {
    program.ends_at >= now - HISTORY && program.starts_at <= now + FUTURE
}

pub(super) fn prune(
    conn: &mut SqliteConnection,
    now: i64,
) -> QueryResult<usize> {
    diesel::delete(
        p::table.filter(
            p::ends_at
                .lt(now - HISTORY)
                .or(p::starts_at.gt(now + FUTURE)),
        ),
    )
    .execute(conn)
}
