import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/react/shallow';
import { Table, TableBody } from '@/components/ui/table';
import type { PinData } from '@/shared/types/store/graph';
import { getNodeDefinitionMeta } from '@/shared/types/domain/node';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import { DetailPanelShell } from '../shared/DetailPanelShell';
import { NodeDocumentationPanel } from '../node/NodeDocumentationPanel';
import { NodePinInterfacePanel } from '../node/NodePinInterfacePanel';
import { resolveNodeDocumentationContent } from '../nodeDocumentation';
import { resolveNodePinSpecs } from '../resolveNodePinSpecs';
import { DetailFieldRow } from '../shared/DetailFieldRow';
import { detailTableClass, detailValueMutedClass } from '../shared/detailStyles';

const EMPTY_PINS: PinData[] = [];

interface NodeDetailPanelProps {
  nodeId: string;
  graphId: string;
}

export function NodeDetailPanel({ nodeId, graphId }: NodeDetailPanelProps) {
  const { t, i18n } = useTranslation();
  const node = useGraphDataStore((s) => s.nodes[nodeId]);
  const pins = useGraphDataStore(
    useShallow((s) => {
      const pinIds = s.nodePins[nodeId];
      if (!pinIds?.length) return EMPTY_PINS;
      return pinIds.map((pid) => s.pins[pid]).filter(Boolean);
    }),
  );
  const nodeType = node?.nodeType;
  const definition = useNodeRegistryStore((s) =>
    nodeType ? s.definitions.get(nodeType) : undefined,
  );

  const pinSpecs = useMemo(
    () => resolveNodePinSpecs(nodeId, pins, definition),
    [nodeId, pins, definition],
  );

  const documentation = useMemo(() => {
    const meta = getNodeDefinitionMeta(definition);
    return resolveNodeDocumentationContent(meta, i18n.language, node?.description);
  }, [definition, node?.description, i18n.language]);

  if (!node) {
    return (
      <DetailPanelShell title={t('detail.titleNode')}>
        <div className={`p-4 text-[11px] ${detailValueMutedClass}`}>{t('detail.nodeNotFound')}</div>
      </DetailPanelShell>
    );
  }

  return (
    <DetailPanelShell title={t('detail.titleWithName', { name: node.title || node.nodeType })}>
      <Table className={detailTableClass}>
        <TableBody>
          <DetailFieldRow label={t('detail.fields.name')} valueClassName="font-medium text-foreground">
            {node.title}
          </DetailFieldRow>
          <DetailFieldRow
            label={t('detail.fields.type')}
            valueClassName={`font-mono text-[10px] ${detailValueMutedClass}`}
          >
            {node.nodeType}
          </DetailFieldRow>
          {node.category?.length > 0 && (
            <DetailFieldRow label={t('detail.fields.category')} valueClassName={detailValueMutedClass}>
              {node.category.join(' / ')}
            </DetailFieldRow>
          )}
          <DetailFieldRow
            label={t('detail.fields.graph')}
            valueClassName="font-mono text-[10px] text-muted-foreground/70"
          >
            {graphId}
          </DetailFieldRow>
        </TableBody>
      </Table>
      <NodePinInterfacePanel inputs={pinSpecs.inputs} outputs={pinSpecs.outputs} />
      {documentation && <NodeDocumentationPanel markdown={documentation} />}
    </DetailPanelShell>
  );
}
