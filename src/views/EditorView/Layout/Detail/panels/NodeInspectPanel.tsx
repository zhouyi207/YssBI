import { useTranslation } from 'react-i18next';
import { useGraphRead } from '@/features/core/graph/read';
import type { NodeData } from '@/shared/types/store/graph';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { DetailForm } from '../shared/DetailForm';
import { DetailText } from '../shared/DetailText';
import { NodeParameterEditor } from '../node/parameterEditors/NodeParameterEditor';

function formatProjectedValue(value: unknown): string {
  if (value == null) return '—';
  return typeof value === 'string' ? value : JSON.stringify(value);
}

export function NodeInspectPanel({ graphPath, nodeId }: { graphPath: string; nodeId: string }) {
  const { t, i18n } = useTranslation();
  const projectedNode = useGraphRead((snapshot) => snapshot.graphEntities[graphPath]?.nodes[nodeId]);
  const node = projectedNode
    ? structuredClone(projectedNode) as unknown as NodeData
    : undefined;

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
