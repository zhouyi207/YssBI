/**
 * InfoView 统一样式（shadcn token），替代 legacy `bg-[#13151a]` / `text-gray-*` 硬编码。
 */
export const infoCard = 'rounded-lg border border-border bg-card overflow-hidden';
export const infoCardPadded = 'rounded-lg border border-border bg-card p-4';
export const infoCardRoundedXl = 'rounded-xl border border-border bg-card overflow-hidden shadow-sm';

export const infoSurfaceMuted = 'bg-muted';
export const infoSurfaceNested = 'bg-muted border border-border';
export const infoRowStripeA = 'bg-card';
export const infoRowStripeB = 'bg-muted/40';
export const infoRowHover = 'hover:bg-muted';

export const infoTableHead = 'bg-muted';
export const infoTableBorder = 'border-border';
export const infoTableRowBorder = 'border-t border-border';

export const infoLabel = 'text-muted-foreground';
export const infoValue = 'text-foreground font-mono';
export const infoHeading = 'text-foreground';
export const infoSectionTitle = 'text-sm font-semibold text-foreground uppercase tracking-wider';

export const infoInput =
  'rounded-md bg-muted border border-border text-sm font-mono text-foreground placeholder:text-muted-foreground focus:outline-none focus:border-[var(--accent-color)]/50';

export function infoTableRowClass(index: number, extra = ''): string {
  const stripe = index % 2 === 0 ? infoRowStripeA : infoRowStripeB;
  return `${infoTableRowBorder} transition-colors ${infoRowHover} ${stripe}${extra ? ` ${extra}` : ''}`;
}
