import { useTranslation } from 'react-i18next';
import { SidebarEmptyState } from '../sections/SidebarEmptyState';
import { SidebarTabPanel } from '../sections/SidebarTabPanel';

export function SidebarNodesTab() {
  const { t } = useTranslation();
  return (
    <SidebarTabPanel>
      <SidebarEmptyState
        title={t('sidebar.nodeCatalogUnavailable')}
        description={t('sidebar.nodeCatalogUnavailableDescription')}
      />
    </SidebarTabPanel>
  );
}
