import { useTranslation } from 'react-i18next';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useEditorStore, type RightSidebarTab } from '@/features/core/editor';
import { DetailsPane } from './DetailsPane';
import { InspectPane } from './InspectPane';
import { ResultPane } from './result/ResultPane';

export function Detail() {
  const { t } = useTranslation();
  const activeTab = useEditorStore((state) => state.rightSidebarTab);
  const setActiveTab = useEditorStore((state) => state.setRightSidebarTab);

  return (
    <div className="right-sidebar-container flex h-full w-full select-none flex-col overflow-hidden bg-[var(--sidebar-bg)]">
      <Tabs
        value={activeTab}
        onValueChange={(value) => setActiveTab(value as RightSidebarTab)}
        className="min-h-0 flex-1 gap-0"
      >
        <div className="flex h-[var(--titlebar-height)] shrink-0 items-end border-b border-border bg-background px-2">
          <TabsList variant="line" className="h-full w-full justify-start gap-0 p-0">
            {(['details', 'inspect', 'result'] as const).map((tab) => (
              <TabsTrigger key={tab} value={tab} className="h-full flex-none rounded-none px-3">
                {t(`detail.tabs.${tab}`)}
              </TabsTrigger>
            ))}
          </TabsList>
        </div>
        <TabsContent value="details" className="min-h-0 overflow-hidden"><DetailsPane /></TabsContent>
        <TabsContent value="inspect" className="min-h-0 overflow-hidden"><InspectPane /></TabsContent>
        <TabsContent value="result" className="min-h-0 overflow-hidden"><ResultPane /></TabsContent>
      </Tabs>
    </div>
  );
}
