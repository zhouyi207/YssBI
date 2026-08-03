import { memo } from 'react';
import { useTranslation } from 'react-i18next';
import { MarkdownRenderer } from '@/shared/ui/MarkdownRenderer';
import { detailProseClass } from '../shared/detailStyles';
import { DetailCollapsibleSection } from '../shared/DetailCollapsibleSection';

interface NodeDocumentationPanelProps {
  markdown: string;
}

export const NodeDocumentationPanel = memo(function NodeDocumentationPanel({
  markdown,
}: NodeDocumentationPanelProps) {
  const { t } = useTranslation();

  return (
    <DetailCollapsibleSection title={t('detail.nodeDoc.documentation')} defaultOpen>
      <div className={detailProseClass}>
        <MarkdownRenderer markdown={markdown} />
      </div>
    </DetailCollapsibleSection>
  );
});
