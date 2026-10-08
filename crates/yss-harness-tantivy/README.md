# Tantivy knowledge index

> Status: Current
> Scope: Rebuildable, in-memory BM25 knowledge search adapter
> Canonical owners: [index](src/index.rs), [document collector](src/collector.rs), [tokenizer](src/tokenizer.rs)
> Update when: Index construction, ranking, tokenization or adapter lifetime changes

`TantivyKnowledgeIndex` implements the platform-neutral
[Knowledge index ports](../yss-harness-contract/src/knowledge_index.rs).
[Harness Core](../yss-harness-core/README.md) owns source records, snapshot generations,
project/sensitivity/scope checks and current citation validation. This crate owns
only an immutable search projection. It has no filesystem, GUI, model or session state.

`build` receives captured chunks and constructs an in-memory index on Tokio's blocking
pool. The returned reader retains that index; replacing Core's prepared snapshot releases
the old projection when its last reader is dropped. Searches also run on the blocking pool.
The index is rebuildable from its source owner and is never a second persistent authority.
Adapter and blocking-task failures map to `KnowledgeIndexFailure`.

Queries use the same mixed-text tokenizer as indexing: lowercase alphanumeric/underscore
words and overlapping Han bigrams, with a single token for an isolated Han character.
The stream borrows the input and retains one token and one overlapping character;
it does not materialize an entire character or token array. Offsets refer to original
UTF-8 bytes, including when lowercase text differs from the source.

The allowed document IDs supplied by Core filter matches before ranking.
BM25 boosts title, metadata and body by 4, 2 and 1 respectively. The collector keeps
one best passage per document across segments before applying the result limit;
equal document scores use document ID order, and equal passage scores use the address
within that index. Excerpts use the selected body with a snippet length target of 480;
an empty generated fragment falls back to its first 480 Unicode characters.
Core rechecks the current source and passage
before exposing a citation, including changes during index construction or search.
Empty document filters, zero limits and text with no tokens return no hits.

Focused validation from the repository root:

```sh
cargo test -p yss-harness-tantivy --lib
cargo test -p yss-harness-core --lib knowledge::tests::
cargo test -p yss-application --lib harness::knowledge::tests::
cargo clippy -p yss-harness-tantivy --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-harness-tantivy --check
```
