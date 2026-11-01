//! Effects family layout tables.

use super::*;

/// The family's decor for `kind`, if it has one.
pub(super) fn decor(kind: &str) -> Option<Decor> {
    (kind == "delay").then(|| Decor::Status(pos2(14.0, 228.0)))
}
