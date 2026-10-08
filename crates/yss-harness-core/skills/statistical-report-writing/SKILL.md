# Statistical Report Writing

Apply this skill when drafting, revising, or continuing a statistical report,
including report text in Assistant replies and Markdown Doc resources. Resolve
follow-up requests using the conversation, not just keywords in the latest message.
For unrelated tasks, keep the normal Assistant behavior. Follow the user's language,
requested scope, and report structure.

## Delivery

- A request to generate, write, or output an analysis/statistical report defaults
  to a saved project Doc, even when the user does not say "file" or "document".
  An explicit request for a chat-only answer, no file, or a brief explanation
  overrides this default.
- Manager delegates document delivery to ReportAgent with exact Doc creation or
  edit/save permissions and the required evidence references. Derive a suitable
  title from the requested analysis when the user does not supply one. Do not
  treat creating the requested Doc as optional follow-up work.
- ReportAgent creates the Doc with `create_resource` or inspects an existing Doc,
  initializes Markdown with `write_document`, extends it with `append_document`, or revises unique passages with `replace_document_text`, then explicitly calls `save_resource`.
  The host binds read and committed facts. Completion requires a successful save after the final edit
  for every changed Doc. If blocked, report the blocker instead of claiming
  delivery or pasting the full requested report into chat.
- After successful delivery, Manager opens the returned Doc resource through
  the UI intent tool and gives a brief summary, document location, and material
  limitations in chat. The worker's final message summarizes its task and
  delivery; the report body belongs in the Doc.

## Evidence and reporting

- Use current tool results and explicitly supplied data. Inspect missing result
  content with `inspect_result`; result IDs and summaries alone are not numerical
  evidence. Reuse available evidence without rerunning an analysis unnecessarily.
- Do not invent estimates, standard errors, p-values, confidence intervals, sample
  sizes, diagnostics, citations, or model specifications. Missing or uncomputed
  values are unknown, not zero. State gaps or retrieve the needed evidence.
- Identify the sample, outcome, predictors, model, units, and uncertainty when the
  evidence supports them. Distinguish associations from causal conclusions. Do not
  infer significance from rounded values or turn a displayed zero into `p = 0`.
- Preserve numerical meaning when rounding or revising prose. Use significance
  stars only with supported thresholds and an explicit legend.

## Markdown and mathematical notation

Use GFM Markdown for report text and LaTeX source for mathematical notation.
Use `$...$` for inline math source and `$$...$$` for standalone math source.
Use `~~text~~` for strikethrough and code spans for literal identifiers.
The active viewer owns rendering; a successful document write establishes saved
source content, without proving that formulas have been rendered.

- Use inline math inside sentences and table cells, for example `$x_i$`, `$R^2$`,
  `$\hat{\beta}_{1}$`, and `$\frac{a}{b}$`. Use braces for compound indices and
  exponents. Use display math for standalone equations, with each `$$` delimiter
  on its own line and blank lines around the block. Do not put display math in cells.
- Put programming notation in code: `y ~ x1 + x2`, `df$y`, `beta_hat`, and `a * b`.
  Inside math, distribution notation uses `\sim`, as in `$Y \sim N(0,1)$`;
  a bare `~` in TeX is spacing, not the distribution symbol.
- Escape currency dollars in prose: `\$100` and `\$200`. Ordinary percentages such
  as `95%` are literal text; inside math write `$95\%$`, since an unescaped `%`
  starts a TeX comment. Do not wrap ordinary currency in math delimiters.
- For literal significance markers use `\*`, `\*\*`, or `\*\*\*`, or a math
  superscript such as `$0.25^{***}$`. A legend can say `\* $p < 0.05$`.
  Avoid accidental emphasis, `* ` list markers, and a standalone `***` rule.
  Keep intentional bold, italics, and lists as normal Markdown.
- Use complete comparisons such as `$p < 0.05$` or `$t = -2.31$`. Do not start a
  plain-text comparison with `> ` or separate a negative sign from its number,
  which can create a blockquote or list. Plain Unicode symbols such as α, β, ≤,
  ≥, ±, and × may remain literal when mathematical layout is unnecessary.
- Use code spans for literal identifiers and syntax. If code itself contains
  backticks, use a longer matching backtick delimiter. Use fenced code blocks with
  a language for actual code. Do not enclose a whole report in a code fence unless
  the user asks for raw Markdown source.

## Tables

- Use a GFM header, separator row, and consistent column counts. Keep each row on
  one source line. Use short headers with units and put explanations after the table.
- A literal pipe inside a cell must be written as `\|`, even inside inline code.
  For math, prefer `$P(A \mid B)$`, `$\lvert x\rvert$`, or `$\lVert x\rVert$`.
  Do not write bare pipes inside cell formulas: GFM splits cells before math parses.
- Use inline fractions and indices without manual HTML, spacer rows, `<br>`, fixed
  widths, or CSS. The renderer owns centering, responsive widths, and math height.
  Move long equations below the table and refer to them in a short cell label.

## Before delivery

Check evidence, numerical consistency, significance legends, balanced math/code
delimiters, table column counts, literal pipes, currency, and TeX percentages.
Escape only the syntax needed in its context; never run blanket substitutions over
the report, code blocks, or formulas.

Use `create_resource`, `inspect_document`, `read_document`, `search_document`,
`write_document`, `append_document`, `replace_document_text`, and `save_resource`
with the exact resource identity from successful receipts. Read additional
contents only as needed. Preserve unrelated content. Claim creation, edits, or saving only
after the corresponding successful receipt. This skill supplies writing
instructions; runtime role/task scope and Gateway checks enforce tool authority.
