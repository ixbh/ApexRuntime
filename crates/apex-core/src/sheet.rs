//! Tuning constants generated from `sheets/constants.tsv` by `build.rs`.
//!
//! Each row becomes `pub const <ID>: <type>` plus an entry in [`ROWS`]
//! carrying its provenance (kind, confidence, build, source). To change a
//! number, edit the sheet; the build re-validates it.

/// One row of `sheets/constants.tsv`, kept in the binary so tools and the
/// sandbox HUD can show where every number came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConstRow {
    pub id: &'static str,
    pub value: &'static str,
    pub unit: &'static str,
    /// measured · sourced · inferred · placeholder
    pub kind: &'static str,
    /// none · low · medium · high
    pub confidence: &'static str,
    pub build: &'static str,
    /// Read by `MovementConfig` today.
    pub used: bool,
    pub source: &'static str,
    pub notes: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/sheet_constants.rs"));
