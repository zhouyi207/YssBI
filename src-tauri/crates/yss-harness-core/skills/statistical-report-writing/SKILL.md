# Statistical Report Writing

Apply this skill when drafting, revising, or continuing a statistical report,
including report text in Assistant replies and Markdown Doc resources. Resolve
follow-up requests using the conversation, not just keywords in the latest message.
For unrelated tasks, keep the normal Assistant behavior. Follow the user's language,
requested scope, and report structure.

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

The shared renderer uses GFM, KaTeX, Typography, and Shiki. Single `~` is literal;
`~~text~~` is strikethrough. `$...$` is inline math; `$$...$$` is centered display
math. Code spans have no decorative backticks. These are renderer rules, not text
substitutions to repeat in a report.

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

When the user requests writing a Doc, use the existing `inspect_resource`,
`manage_resource`, and `edit_resource` lifecycle with current resource identity and
revision. Preserve unrelated content. Claim creation, edits, or saving only after
the corresponding successful receipt. This skill supplies writing instructions;
it does not grant tools or authorize additional project changes.
