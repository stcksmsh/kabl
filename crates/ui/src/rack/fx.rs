//! Effects family layout tables.

use super::*;

/// The family's decor for `kind`, if it has one.
pub(super) fn decor(kind: &str) -> Option<Decor> {
    (kind == "delay").then(|| Decor::Status(pos2(14.0, 228.0)))
}

/// Short face label of `param` for the family's kinds (none yet).
pub(super) fn short_name(_kind: &str, _param: &str) -> Option<&'static str> {
    None
}
