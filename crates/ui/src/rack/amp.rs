//! Amplifier family layout tables.

use super::*;

/// The family's decor for `kind`, if it has one.
pub(super) fn decor(kind: &str, fw: f32) -> Option<Decor> {
    (kind == "out").then(|| Decor::Speaker(pos2(fw / 2.0, 112.0)))
}

/// Short face label of `param` for the family's kinds (none yet).
pub(super) fn short_name(_kind: &str, _param: &str) -> Option<&'static str> {
    None
}
