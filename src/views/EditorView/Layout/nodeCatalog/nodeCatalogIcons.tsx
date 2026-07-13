import { VscSymbolMethod, VscSymbolProperty, VscSymbolVariable } from 'react-icons/vsc';
import type { NodeCatalogItem } from '@/features/domain/nodeCatalog';
import { SIDEBAR_ROW_ICON_SIZE } from '../sidebarUi/sidebarStyles';

export function nodeCatalogItemIcon(item: NodeCatalogItem) {
  if (item.nodeType.includes('Variable')) {
    return <VscSymbolVariable className="text-blue-400/90" size={SIDEBAR_ROW_ICON_SIZE} />;
  }
  if (item.nodeType.includes('Call')) {
    return <VscSymbolMethod className="text-purple-400/90" size={SIDEBAR_ROW_ICON_SIZE} />;
  }
  return <VscSymbolProperty className="text-[var(--accent-color)]" size={SIDEBAR_ROW_ICON_SIZE} />;
}
