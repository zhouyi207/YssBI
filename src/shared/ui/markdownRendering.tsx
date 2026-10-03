import { useEffect, useState } from "react";
import type { Components, Options } from "react-markdown";
import type { Root } from "mdast";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { logger } from "@/utils/frontendLogger";
import "katex/dist/katex.min.css";
import { MarkdownLink } from "./MarkdownLink";

function remarkMathLayout() {
  return (tree: Root, file: { toString(): string }) => {
    const source = file.toString();

    function visit(node: Root | Root["children"][number]) {
      const start = node.position?.start.offset;
      // remark-math parses same-line $$...$$ as inline math; our delimiter sets display mode.
      if (node.type === "inlineMath" && start !== undefined && source.startsWith("$$", start)) {
        node.data = {
          ...node.data,
          hProperties: {
            ...node.data?.hProperties,
            className: ["language-math", "math-display"],
          },
        };
      }
      if ("children" in node) node.children.forEach(visit);
    }

    visit(tree);
  };
}

export const markdownRemarkPlugins: NonNullable<Options["remarkPlugins"]> = [
  // Regression model notation uses single tildes; reserve strikethrough for double tildes.
  [remarkGfm, { singleTilde: false }],
  [remarkMath, { singleDollarTextMath: true }],
  remarkMathLayout,
];
const REHYPE_PLUGINS: NonNullable<Options["rehypePlugins"]> = [rehypeKatex];
export const markdownComponents: Components = {
  a: MarkdownLink,
  table: ({ children }) => (
    <div className="markdown-table-scroll" tabIndex={0}>
      <table>{children}</table>
    </div>
  ),
};

export function useMarkdownRehypePlugins() {
  const [rehypePlugins, setRehypePlugins] = useState(REHYPE_PLUGINS);

  useEffect(() => {
    let active = true;
    void import("./markdownHighlight")
      .then(({ rehypeHighlight }) => {
        if (active) setRehypePlugins([...REHYPE_PLUGINS, rehypeHighlight]);
      })
      .catch(() => {
        logger.app.warn(
          "Code highlighting could not load; keeping plain text.",
          "MarkdownRenderer",
        );
      });
    return () => {
      active = false;
    };
  }, []);

  return rehypePlugins;
}
