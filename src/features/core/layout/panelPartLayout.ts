export const PANEL_POSITION_SETTINGS = ['Bottom', 'Left', 'Right'] as const;
export type PanelPositionSetting = (typeof PANEL_POSITION_SETTINGS)[number];
export type PanelPosition = 'bottom' | 'left' | 'right';

export function normalizePanelPosition(value: string | undefined): PanelPosition {
  switch (value?.toLowerCase()) {
    case 'left': return 'left';
    case 'right': return 'right';
    default: return 'bottom';
  }
}

export function panelPositionToSetting(position: PanelPosition): PanelPositionSetting {
  if (position === 'left') return 'Left';
  if (position === 'right') return 'Right';
  return 'Bottom';
}

export function isPanelPositionHorizontal(position: PanelPosition): boolean {
  return position === 'left' || position === 'right';
}
