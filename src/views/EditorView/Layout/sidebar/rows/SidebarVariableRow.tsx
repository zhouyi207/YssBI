import { memo } from 'react';
import { VscSymbolVariable } from 'react-icons/vsc';
import { buildSidebarDragData } from '@/features/application/sidebar';
import { focusDetail } from '@/features/core/editor/detail/detailFocusCommands';
import { TYPE_ICON_COLORS } from '@/features/domain/sidebar';
import type { DataType } from '@/shared/types/domain/dataType';
import { safeDataTypeColor, safeDataTypeDisplay } from '../../sidebarUtils';
import {
  SidebarListItem,
  sidebarVariableTypeBadgeClass,
  SIDEBAR_ROW_ICON_SIZE,
} from '../../sidebarUi';

export const SidebarVariableRow = memo(function SidebarVariableRow({
  id,
  name,
  dataType,
  isGlobal,
  indentDepth = 0,
  isSelected = false,
  onContextMenu,
}: {
  id: string;
  name: string;
  dataType: unknown;
  isGlobal: boolean;
  indentDepth?: number;
  isSelected?: boolean;
  onContextMenu: (e: React.MouseEvent) => void;
}) {
  const iconColor = isGlobal ? TYPE_ICON_COLORS.variableGlobal : TYPE_ICON_COLORS.variable;

  return (
    <SidebarListItem
      id={id}
      dragData={buildSidebarDragData(id, name, 'variable')}
      isSelected={isSelected}
      indentDepth={indentDepth}
      icon={<VscSymbolVariable size={SIDEBAR_ROW_ICON_SIZE} style={{ color: iconColor }} />}
      label={name}
      onClick={(e) => {
        e.stopPropagation();
        focusDetail({ kind: 'variable', id });
      }}
      onContextMenu={onContextMenu}
      trailing={
        <span
          className={sidebarVariableTypeBadgeClass(isSelected)}
          style={{ color: safeDataTypeColor(dataType) }}
        >
          {safeDataTypeDisplay(dataType)}
          {dataType &&
          typeof dataType === 'object' &&
          'kind' in dataType &&
          (dataType as DataType).kind === 'Array'
            ? <span className="text-[8px]">[]</span>
            : null}
        </span>
      }
    />
  );
});
