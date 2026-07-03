export type { DetailTarget, DetailTargetInput, DetailFocus } from './types';
export { resolveDetailTarget } from './resolveDetailTarget';
export { useDetailTarget } from './useDetailTarget';
export { clearDetailFocusForClosedTab } from './clearDetailFocusForClosedTab';
export {
  applyCanvasDetailFocus,
  focusDetail,
  focusDetailOnActiveGraph,
  focusDetailOnNode,
} from './detailFocusCommands';
export type { CanvasDetailGesture } from './detailFocusCommands';
export {
  syncVariablesGraphScopeAfterClose,
  syncVariablesGraphScopeFromActiveTab,
  setVariablesGraphScopeFromResource,
} from './variablesGraphScope';
