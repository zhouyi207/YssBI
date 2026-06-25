import { memo } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';
import 'katex/dist/katex.min.css';

interface NodeDocumentationPanelProps {
  markdown: string;
}

const REMARK_PLUGINS = [remarkMath];
const REHYPE_PLUGINS = [rehypeKatex];

export const NodeDocumentationPanel = memo(function NodeDocumentationPanel({
  markdown,
}: NodeDocumentationPanelProps) {
  return (
    <div className="border-t border-white/5 px-3 py-3">
      <div className="mb-2 text-[10px] font-black uppercase tracking-widest text-gray-500">
        Documentation
      </div>
      <div className="prose prose-invert max-w-none text-[11px] leading-relaxed text-gray-300 [&_h1]:text-sm [&_h1]:font-bold [&_h2]:text-xs [&_h2]:font-semibold [&_h3]:text-[11px] [&_p]:my-2 [&_table]:text-[10px] [&_td]:border [&_td]:border-white/10 [&_td]:px-2 [&_td]:py-1 [&_th]:border [&_th]:border-white/10 [&_th]:px-2 [&_th]:py-1 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-4 [&_.katex]:text-gray-200">
        <ReactMarkdown remarkPlugins={REMARK_PLUGINS} rehypePlugins={REHYPE_PLUGINS}>
          {markdown}
        </ReactMarkdown>
      </div>
    </div>
  );
});
