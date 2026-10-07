import { memo } from "react";
import ReactMarkdown from "react-markdown";
import {
  markdownComponents,
  markdownRemarkPlugins,
  useMarkdownRehypePlugins,
} from "./markdownRendering";

export const MarkdownRenderer = memo(function MarkdownRenderer({ markdown }: { markdown: string }) {
  const rehypePlugins = useMarkdownRehypePlugins();

  return (
    <ReactMarkdown
      remarkPlugins={markdownRemarkPlugins}
      rehypePlugins={rehypePlugins}
      components={markdownComponents}
    >
      {markdown}
    </ReactMarkdown>
  );
});
