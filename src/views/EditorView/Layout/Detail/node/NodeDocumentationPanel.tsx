import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkMath from 'remark-math';
import rehypeKatex from 'rehype-katex';
import 'katex/dist/katex.min.css';
import { detailProseClass } from '../shared/detailStyles';
import { DetailCollapsibleSection } from '../shared/DetailCollapsibleSection';

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
    <DetailCollapsibleSection title={t('detail.nodeDoc.documentation')} defaultOpen>
      <div className={detailProseClass}>
        <ReactMarkdown remarkPlugins={REMARK_PLUGINS} rehypePlugins={REHYPE_PLUGINS}>
          {markdown}
        </ReactMarkdown>
      </div>
    </DetailCollapsibleSection>
  );
});
