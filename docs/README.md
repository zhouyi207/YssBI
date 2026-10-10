# YssBI Docs

YssBI's documentation is a [mdBook](https://rust-lang.github.io/mdBook/) with task-oriented pages and a single [table of contents](src/SUMMARY.md).

The organization follows [Zed's docs](https://github.com/zed-industries/zed/tree/main/docs). The instructions describe YssBI, not Zed's editor, services or release process. YssBI has no published documentation site yet.

## Preview locally

Install the same mdBook version used by Zed and the math preprocessor for node help:

```sh
cargo install mdbook --version 0.4.40 --locked
cargo install mdbook-katex --version 0.9.0 --locked
```

Building `mdbook-katex` from source also requires a C compiler and `patch` on `PATH`.

From the repository root, start the preview:

```sh
mdbook serve docs --hostname 127.0.0.1 --port 3000
```

Open `http://127.0.0.1:3000`. Changes to the source pages rebuild the book. You do not need to compile the desktop application or generate action metadata.

To build without starting a server:

```sh
mdbook build docs
```

Generated files go to `docs/book/`, which is ignored by Git. The book uses mdBook's standard HTML renderer and bundled theme; no Zed-specific preprocessor, analytics, redirects or deployment service is required.

`mdbook-katex` renders the node help's `$...$` and `$$...$$` formulas at build time. Its default stylesheet and fonts load from a CDN, so full formula styling requires a network connection when browsing.

## Edit the documentation

- Write book pages in `src/` and add them to `src/SUMMARY.md`.
- Use lowercase, hyphen-separated filenames and relative links between pages. Existing node help retains its catalog slug filenames.
- Follow [.rules](.rules) and the [documentation conventions](.conventions/CONVENTIONS.md).
- Keep module implementation contracts in the module's README and open work in the repository's [TODO.md](../TODO.md).
- Do not add historical test transcripts, benchmark output dumps or completed roadmaps to the book.

## Node help

Maintain bilingual node help in `src/nodes/en/` and `src/nodes/zh/`, following the [node documentation rules](src/nodes/.rules). Register both language pages beneath [Node Reference](src/nodes.md) in `src/SUMMARY.md`.

These are the canonical Markdown files for both the book and the desktop application's embedded help. `yss-node-catalog` includes them at compile time; do not keep copies under the crate. A book preview rebuilds when you edit the Markdown, while the application needs to be rebuilt to embed those edits. Catalog mappings and generated help remain owned by the [catalog module](../crates/yss-node-catalog/README.md).

## Format and check

From `docs/`, run:

```sh
npx --yes prettier@3.5.0 . --check
npx --yes prettier@3.5.0 --parser markdown .rules --check
```

Replace `--check` with `--write` to format changed documentation. Node.js is only needed for formatting and the existing React reference-data generator, not for the application or mdBook itself.

From the repository root, run `mdbook build docs` and `git diff --check`. Inspect the generated navigation and links, including heading anchors. A successful book build does not validate external URLs or demonstrate that an application workflow has passed acceptance.
