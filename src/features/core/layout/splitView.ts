/**
 * VS Code SplitView sizing primitives — shared by workbench sash and editor grid.
 * Imperative drag lives in `sashResizeLogic`; this module owns flex math only.
 */
export {
  computeFlexSplitSizes,
  isFlexSplitPair,
  type FlexSplitPair,
} from './splitViewSizing';

export { equalSplitPairSizes } from './editorGridLayout';

/** flex: 0 0 Npx — single-axis split child (VS Code split-view). */
export function panelFlexBasis(sizePx: number): string {
  return `0 0 ${sizePx}px`;
}
