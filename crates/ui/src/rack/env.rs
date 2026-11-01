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

/// Params the family's face shows although the module lists them as advanced.
pub(super) fn on_face(_kind: &str, _param: &str) -> bool {
    false
}

/// A family knob radius override for `param`.
pub(super) fn knob_size(_kind: &str, _param: &str) -> Option<f32> {
    None
}

/// ADSR (R1): the CONT / KEY choice sits in the header.
pub(super) fn in_header(kind: &str, param: &str) -> bool {
    kind == "env.adsr" && param == "timing"
}
