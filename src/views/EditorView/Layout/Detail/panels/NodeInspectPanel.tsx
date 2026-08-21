import { useTranslation } from 'react-i18next';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailForm } from '../shared/DetailForm';
import { DetailText } from '../shared/DetailText';
import { NodeParameterEditor } from '../node/parameterEditors/NodeParameterEditor';
import { selectNodeDetailNode } from './NodeDetailPanel';

function formatProjectedValue(value: unknown): string {
  if (value == null) return '—';
  return typeof value === 'string' ? value : JSON.stringify(value);
}

export function NodeInspectPanel({ graphPath, nodeId }: { graphPath: string; nodeId: string }) {
  const { t, i18n } = useTranslation();
  const node = useGraphDataStore((state) => selectNodeDetailNode(state, graphPath, nodeId));

  if (!node) {
    return (
      <DetailPanelShell>
        <DetailText as="div" tone="muted" className="p-4">
          {t('detail.nodeNotFound')}
        </DetailText>
      </DetailPanelShell>
    );
  }

  if (!node.parameterEditors?.length) {
    return (
      <DetailPanelShell>
        <DetailText as="div" tone="muted" className="p-4">
          {t('detail.inspect.noParameters')}
        </DetailText>
      </DetailPanelShell>
    );
  }

  return (
    <DetailPanelShell>
      <DetailForm>
        {node.parameterEditors.map((parameter) => (
          <NodeParameterEditor
            key={parameter.key}
            graphPath={graphPath}
            nodeId={nodeId}
            locale={i18n.language}
            parameter={parameter}
            diagnostics={node.diagnostics ?? []}
            formatFallback={formatProjectedValue}
          />
        ))}
      </DetailForm>
    </DetailPanelShell>
  );
}
