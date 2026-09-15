# Dataset Quality Review

Use this skill before statistical estimation when the dataset schema and quality must be reviewed.

Required inputs:

- a project-bound dataset identity;
- its current schema revision;
- the research question or intended analysis.

Method:

1. Inspect the dataset schema and identify measurement types.
2. Inspect the revision-bound dataset profile for shape, duplicates, and missingness.
3. Check whether outcome, predictor, grouping, time, and identifier roles are well-defined.
4. Require missingness, duplicate-key, range, and outlier diagnostics before estimation.
5. Distinguish structural missingness from unexpected missing values.
6. Stop if the dataset revision changes or required variable semantics are unknown.
7. Report limitations and unresolved semantics; do not invent values or numerical evidence.

This skill may inspect project facts but may not mutate the project, query raw rows, call external services, or write persistent Memory directly.
