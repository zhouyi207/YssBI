import { useTranslation } from 'react-i18next';
import { VscPreview } from 'react-icons/vsc';
import { useShallow } from 'zustand/react/shallow';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import {
  useResultWorkspaceStore,
  type ResultTabRecord,
} from '@/features/core/resultWorkspace';
import { ResultContent } from './ResultContent';
import { ResultTabStrip } from './ResultTabStrip';

export function ResultPane() {
  const { t } = useTranslation();
  const {
    order,
    tabs,
    activeTabKey,
    setActiveTab,
    closeTab,
    moveTab,
  } = useResultWorkspaceStore(useShallow((state) => ({
    order: state.order,
    tabs: state.tabs,
    activeTabKey: state.activeTabKey,
    setActiveTab: state.setActiveTab,
    closeTab: state.closeTab,
    moveTab: state.moveTab,
  })));
  const records = order
    .map((key) => tabs[key])
    .filter((record): record is ResultTabRecord => Boolean(record));
  const active = activeTabKey ? tabs[activeTabKey] : null;

  if (records.length === 0) {
    return (
      <Empty className="h-full min-h-0 rounded-none p-4">
        <EmptyHeader>
          <EmptyMedia variant="icon" className="size-10 text-muted-foreground">
            <VscPreview className="size-5" />
          </EmptyMedia>
          <EmptyTitle>{t('detail.result.emptyTitle')}</EmptyTitle>
          <EmptyDescription>{t('detail.result.empty')}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col overflow-hidden bg-background/40">
      <ResultTabStrip
        tabs={records}
        activeTabKey={activeTabKey}
        onActivate={setActiveTab}
        onClose={closeTab}
        onMove={moveTab}
      />
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        {active ? (
          <ResultContent
            key={`${active.tabKey}:${active.resultId}`}
            resultId={active.resultId}
          />
        ) : null}
      </div>
    </div>
  );
}
