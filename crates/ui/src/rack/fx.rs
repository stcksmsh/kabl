//! Effects family layout tables.

use super::*;

/// The family's decor for `kind`, if it has one.
pub(super) fn decor(kind: &str) -> Option<Decor> {
    (kind == "delay").then(|| Decor::Status(pos2(14.0, 228.0)))
}

/// Short face label of `param` for the effects (full names stay in the inspector).
pub(super) fn short_name(kind: &str, param: &str) -> Option<&'static str> {
    Some(match (kind, param) {
        ("flanger", "feedback") => "Fdbk",
        ("compressor", "threshold_db") => "Thresh",
        ("bitcrusher", "downsample") => "Rate",
        ("eq", "mid1_hz") => "M1 freq",
        ("eq", "mid1_db") => "M1 gain",
        ("eq", "mid1_q") => "M1 Q",
        ("eq", "mid2_hz") => "M2 freq",
        ("eq", "mid2_db") => "M2 gain",
        ("eq", "mid2_q") => "M2 Q",
        _ => return None,
    })
}
