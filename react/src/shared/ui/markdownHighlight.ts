import rehypeShikiFromHighlighter from "@shikijs/rehype/core";
import { createHighlighterCoreSync } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
import bash from "shiki/langs/bash.mjs";
import css from "shiki/langs/css.mjs";
import html from "shiki/langs/html.mjs";
import javascript from "shiki/langs/javascript.mjs";
import json from "shiki/langs/json.mjs";
import julia from "shiki/langs/julia.mjs";
import python from "shiki/langs/python.mjs";
import r from "shiki/langs/r.mjs";
import rust from "shiki/langs/rust.mjs";
import sql from "shiki/langs/sql.mjs";
import toml from "shiki/langs/toml.mjs";
import tsx from "shiki/langs/tsx.mjs";
import typescript from "shiki/langs/typescript.mjs";
import yaml from "shiki/langs/yaml.mjs";
import githubDark from "shiki/themes/github-dark.mjs";
import githubLight from "shiki/themes/github-light.mjs";
import { logger } from "@/utils/frontendLogger";

// This module is loaded once on demand and shared by all Markdown views.
const highlighter = createHighlighterCoreSync({
  engine: createJavaScriptRegexEngine(),
  themes: [githubLight, githubDark],
  langs: [
    bash,
    css,
    html,
    javascript,
    json,
    julia,
    python,
    r,
    rust,
    sql,
    toml,
    tsx,
    typescript,
    yaml,
  ],
});

export function rehypeHighlight() {
  return rehypeShikiFromHighlighter(highlighter, {
    themes: { light: "github-light", dark: "github-dark" },
    defaultColor: false,
    addLanguageClass: true,
    // Assistant code headers copy the rendered text, including its final newline.
    stripEndNewline: false,
    onError: () => {
      logger.app.warn(
        "A code block could not be highlighted; keeping plain text.",
        "MarkdownRenderer",
      );
    },
  });
}
