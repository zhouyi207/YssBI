/// Canonical per-record content limits shared by sanitization and validation.
pub(super) struct DiagnosticLimits;

impl DiagnosticLimits {
    pub(super) const MAX_TARGET_BYTES: usize = 256;
    pub(super) const MAX_EVENT_BYTES: usize = 256;
    pub(super) const MAX_MESSAGE_BYTES: usize = 16 * 1024;
    pub(super) const MAX_SOURCE_BYTES: usize = 1024;
    pub(super) const MAX_FIELD_STRING_BYTES: usize = 4 * 1024;
    pub(super) const MAX_FIELD_KEY_BYTES: usize = 128;
    pub(super) const MAX_FIELDS_BYTES: usize = 32 * 1024;
    pub(super) const MAX_FIELD_COUNT: usize = 64;
    pub(super) const MAX_FIELD_DEPTH: usize = 8;
    pub(super) const MAX_FIELD_VALUES: usize = 1024;
}
