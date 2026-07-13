import { useTranslation } from 'react-i18next';
import { VscSearch } from 'react-icons/vsc';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';
import { nodeCatalogSearchShellClass } from '../sidebarUi';

export function NodeCatalogSearchBar({
  value,
  onChange,
  className,
}: {
  value: string;
  onChange: (value: string) => void;
  className?: string;
}) {
  const { t } = useTranslation();

  return (
    <div className={cn(nodeCatalogSearchShellClass(), className)}>
      <div
        className={cn(
          'flex items-center gap-1 rounded-md border border-border/60 px-2 py-1 transition-[border-color,box-shadow]',
          'bg-sidebar/40 focus-within:border-sidebar-border focus-within:bg-sidebar/60',
        )}
      >
        <VscSearch className="ml-0.5 shrink-0 text-muted-foreground/70" size={13} aria-hidden />
        <Input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={t('canvas.nodePalette.searchPlaceholder')}
          className="h-7 min-w-0 flex-1 border-0 bg-transparent px-1.5 text-[13px] shadow-none focus-visible:ring-0"
        />
      </div>
    </div>
  );
}
