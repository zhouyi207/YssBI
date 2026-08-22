import { VscAdd } from 'react-icons/vsc';
import { Button } from '@/components/ui/button';
import { SidebarChevron } from '../../sidebarUi/SidebarChevron';
import {
  sidebarGroupRowClass,
  sidebarItemIndent,
  sidebarItemLabelClass,
  SIDEBAR_ROW_ICON_SIZE,
  SIDEBAR_ROW_LEADING_SLOT_CLASS,
  SIDEBAR_ROW_TRAILING_SLOT_CLASS,
} from '../../sidebarUi/sidebarStyles';

export function SidebarGroupRow({
  level,
  label,
  expanded,
  onToggle,
  onAdd,
  addAriaLabel,
  onContextMenu,
}: {
  level: number;
  label: React.ReactNode;
  expanded: boolean;
  onToggle: () => void;
  onAdd?: () => void;
  addAriaLabel?: string;
  onContextMenu?: (e: React.MouseEvent) => void;
}) {
  return (
    <div
      role="button"
      tabIndex={0}
      aria-expanded={expanded}
      onClick={(e) => {
        e.stopPropagation();
        onToggle();
      }}
      onKeyDown={(e) => {
        if (e.key !== 'Enter' && e.key !== ' ') return;
        e.preventDefault();
        onToggle();
      }}
      onContextMenu={onContextMenu}
      className={sidebarGroupRowClass()}
      style={sidebarItemIndent(level)}
    >
      <span className={SIDEBAR_ROW_LEADING_SLOT_CLASS}>
        <SidebarChevron expanded={expanded} />
      </span>
      <span className={sidebarItemLabelClass()}>{label}</span>
      {onAdd ? (
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={addAriaLabel ?? (typeof label === 'string' ? label : undefined)}
          onClick={(e) => {
            e.stopPropagation();
            onAdd();
          }}
          className="size-6 shrink-0 p-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100"
        >
          <VscAdd size={SIDEBAR_ROW_ICON_SIZE} />
        </Button>
      ) : (
        <span className={SIDEBAR_ROW_TRAILING_SLOT_CLASS} aria-hidden />
      )}
    </div>
  );
}
