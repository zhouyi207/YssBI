import { useTranslation } from 'react-i18next';
import { VscSettingsGear } from 'react-icons/vsc';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { useActiveEditorGroup } from '@/features/core/editor';
import { NodeInspectPanel } from './panels/NodeInspectPanel';
import { resolveNodeInspectionTarget } from './resolveNodeInspectionTarget';

function InspectEmpty({
  title,
  description,
}: {
  title: string;
  description: string;
}) {
  return (
    <Empty className="h-full min-h-0 rounded-none p-4">
      <EmptyHeader>
        <EmptyMedia variant="icon" className="size-10 text-muted-foreground">
          <VscSettingsGear className="size-5" />
        </EmptyMedia>
        <EmptyTitle>{title}</EmptyTitle>
        <EmptyDescription>{description}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}

export function InspectPane() {
  const { t } = useTranslation();
  const { activeTabId, tabs, selectedNodeIds } = useActiveEditorGroup();
  const activeTab = tabs.find((tab) => tab.id === activeTabId);
  const graphPath = activeTab?.type === 'event' || activeTab?.type === 'function'
    ? activeTab.id
    : null;
  const target = resolveNodeInspectionTarget(graphPath, selectedNodeIds);

  if (target.kind === 'node') {
    return <NodeInspectPanel graphPath={target.graphPath} nodeId={target.nodeId} />;
  }
  if (target.kind === 'multiple') {
    return (
      <InspectEmpty
        title={t('detail.inspect.multipleTitle', { count: target.count })}
        description={t('detail.inspect.multiple')}
      />
    );
  }
  return (
    <InspectEmpty
      title={t('detail.inspect.emptyTitle')}
      description={t('detail.inspect.empty')}
    />
  );
}
