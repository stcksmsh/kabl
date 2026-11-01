//! Amplifier family layout tables.

use super::*;

/// The family's decor for `kind`, if it has one.
pub(super) fn decor(kind: &str, fw: f32) -> Option<Decor> {
    (kind == "out").then(|| Decor::Speaker(pos2(fw / 2.0, 112.0)))
}
