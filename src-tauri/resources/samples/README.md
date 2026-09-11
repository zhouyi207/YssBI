# Bundled sample data

The CSV files in this directory are maintainer-supplied source inputs. They remain
unchanged when preparing the desktop resources. The flights and vic_elec v2 sources
remove timezone suffixes while retaining the original date and clock fields.
Their original upstream attribution
and licenses have not been specified; filenames alone are not provenance evidence.

The source definitions and display names live in
`../../../scripts/samples/sources.json`. Run `pnpm samples:build` from the repository
root to generate the versioned Parquet files and `catalog.json`. Run
`pnpm samples:check` to verify input/output hashes, row counts, column counts and
catalog drift without changing files. Preparation uses the existing Rust tabular I/O
reader and writer; it inspects the complete CSV during schema inference. Empty CSV
cells keep their null semantics, and source columns, including `rownames`, are retained.
Datetime columns use timezone-free Arrow timestamps; input offsets are removed before
decoding so that the original wall clock is preserved.

Only the catalog, this notice and versioned Parquet files are bundled. Runtime code
never reads the CSV inputs or the source definitions. The catalog is cached after a
successful read, and each selected Parquet is size/hash checked on its open file
before import. Sample imports create ordinary, independent project datasets with
fresh identities. The exact sample version and input/output content hashes are
retained in the dataset's `yssbi.sample.origin` schema metadata.

When a source's content or preparation semantics change, increment its version in
the source definitions and regenerate. Existing project datasets keep their imported
content and do not follow changes to the application resources.
