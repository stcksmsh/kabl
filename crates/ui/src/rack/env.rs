//! Envelope and modulation family layout tables (agent B).

use super::*;

/// The family's decor for `kind`, if it has one.
pub(super) fn decor(_kind: &str) -> Option<Decor> {
    None
}

/// Short face label of `param` for the family's kinds (none yet).
pub(super) fn short_name(_kind: &str, _param: &str) -> Option<&'static str> {
    None
}
