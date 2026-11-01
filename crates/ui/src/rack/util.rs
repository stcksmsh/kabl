//! Utility family layout tables (agent C).

use super::*;

/// The family's decor for `kind`, if it has one.
pub(super) fn decor(_kind: &str) -> Option<Decor> {
    None
}

/// Short face label of a param or jack for the utilities (full names stay in the inspector).
pub(super) fn short_name(kind: &str, name: &str) -> Option<&'static str> {
    Some(match (kind, name) {
        // 'Clock' is 30 px wide against a 28 px label slot (jack pitch 32 - 4).
        ("sample.hold", "clock") => "Clk",
        _ => return None,
    })
}
