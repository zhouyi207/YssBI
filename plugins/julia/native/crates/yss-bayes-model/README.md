# yss-bayes-model

Pure Bayesian model construction and validation shared by transport,
application workflows, and backend adapters.

This crate owns model drafts, expression parsing, validation reports, and the
validated draft-to-spec conversion. It does not own inference task state,
worker capabilities, result artifacts, datasets, Julia processes, or Tauri
transport.

Draft conversion and worker admission reuse the same canonical immutable-spec
validator. Validation reports store only issues; their JSON `ok` field is
derived during serialization and cannot drift from the error set.
