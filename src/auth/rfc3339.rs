//! RFC3339 strings in SQLite; structs use `time::serde::rfc3339`.

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Parse DB text; bad input → `now_utc()`.
pub fn parse(s: &str) -> OffsetDateTime {
    OffsetDateTime::parse(s, &Rfc3339).unwrap_or_else(|_| OffsetDateTime::now_utc())
}

pub fn format(dt: OffsetDateTime) -> String {
    dt.format(&Rfc3339).expect("RFC3339 format")
}
