import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';
import 'katex/dist/katex.min.css';
import { detailProseClass, detailSectionTitleClass } from '../shared/detailStyles';

interface NodeDocumentationPanelProps {
  markdown: string;
}

const REMARK_PLUGINS = [remarkMath];
const REHYPE_PLUGINS = [rehypeKatex];

export const NodeDocumentationPanel = memo(function NodeDocumentationPanel({
  markdown,
}: NodeDocumentationPanelProps) {
  const { t } = useTranslation();

  return (
    <div className="border-t border-border px-3 py-3">
      <div className={`mb-2 ${detailSectionTitleClass}`}>{t('detail.nodeDoc.documentation')}</div>
      <div className={detailProseClass}>
        <ReactMarkdown remarkPlugins={REMARK_PLUGINS} rehypePlugins={REHYPE_PLUGINS}>
          {markdown}
        </ReactMarkdown>
      </div>
    </div>
  );
});
