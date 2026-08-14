
import { useTranslation } from 'react-i18next';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useEditorStore, type DetailPaneTab } from '@/features/core/editor';
import { DetailsPane } from './DetailsPane';
import { InspectorPane } from './InspectorPane';

export function Detail() {
  const { t } = useTranslation();
  const activeTab = useEditorStore((state) => state.detailPaneTab);
  const setActiveTab = useEditorStore((state) => state.setDetailPaneTab);

  return (
    <div
      className="right-sidebar-container flex h-full w-full select-none flex-col overflow-hidden bg-[var(--sidebar-bg)]"
    >
      <Tabs
        value={activeTab}
        onValueChange={(value) => setActiveTab(value as DetailPaneTab)}
        className="min-h-0 flex-1 gap-0"
      >
        <div className="flex h-[var(--titlebar-height)] shrink-0 items-end border-b border-border bg-background px-2">
          <TabsList variant="line" className="h-full w-full justify-start gap-0 p-0">
            <TabsTrigger value="details" className="h-full flex-none rounded-none px-3">
              {t('detail.tabs.details')}
            </TabsTrigger>
            <TabsTrigger value="inspector" className="h-full flex-none rounded-none px-3">
              {t('detail.tabs.inspector')}
            </TabsTrigger>
          </TabsList>
        </div>
        <TabsContent value="details" className="min-h-0 overflow-hidden">
          <DetailsPane />
        </TabsContent>
        <TabsContent value="inspector" className="min-h-0 overflow-hidden">
          <InspectorPane />
        </TabsContent>
      </Tabs>
    </div>
  );
}
