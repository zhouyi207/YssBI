# Documentation Conventions

Use the same task-oriented organization as the [Zed documentation](https://github.com/zed-industries/zed/tree/main/docs), with YssBI's own commands and capabilities.

## What to document

Document user-facing features, configuration, commands and supported development workflows. Update a page when its instructions become inaccurate. Do not add pages for internal refactors, test runs or completed implementation batches.

Create a page for a substantial topic readers look for by name. Put an option or a small addition in its existing feature page.

## Page structure

1. Start with one `#` title and a short explanation of the task.
2. Put the most common workflow first.
3. Describe configuration and important limitations.
4. Link to related tasks when they help the reader continue.

Use `##` sections and `###` subsections. Prefer short paragraphs and concrete steps. Use lowercase, hyphen-separated filenames, except `README.md` and `SUMMARY.md`. Existing node help in `src/nodes/{en,zh}/` retains its catalog slug filenames because those files are also embedded in the application.

The book uses mdBook's standard HTML renderer. Start pages with their Markdown title, not YAML frontmatter or repository-policy metadata. Zed's frontmatter postprocessor, action templates and hosted publishing infrastructure are not part of this book.

## Examples and terminology

- Use `sh` fences for terminal commands and the appropriate language for other code.
- Make examples complete and identify prerequisites. State when a command changes files or requires a project copy.
- Use inline code for paths, commands, settings and key combinations.
- Show platform-specific shortcuts separately when they differ.
- Use tables for options and shortcut comparisons, not long explanations.
- Introduce product terms before implementation details. Do not rename YssBI concepts to match unrelated Zed features.
- Use `> **Note:**` or `> **Warning:**` only for useful constraints or risks.

## Links and navigation

- Add every book page to `src/SUMMARY.md`.
- Link between book pages with relative `.md` paths.
- Use stable explicit heading IDs for sections referenced from other files.
- Link to repository source with GitHub URLs. Relative links outside `src/` do not become usable source pages in the generated book.
- Keep the repository root README as the source-build entry and `docs/README.md` as the documentation-build entry.
- Do not add redirects for this unreleased project; update existing callers directly when moving a page.

## Scope and evidence

Describe current behavior, not an accepted design that has not been implemented. Keep missing capabilities and uncompleted acceptance visible without copying a development backlog into each page. Historical measurements and passing isolated checks are not current end-to-end performance or platform guarantees.
