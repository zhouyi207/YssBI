import { create } from 'zustand';
import { graphOutputKey } from '@/features/domain/editorProjection';
import type {
  GraphOutputRefDto,
  ResultDescriptor,
  ResultPresentation,
} from '@/shared/types/dto/result';

export type ResultTabKey = string;

export interface ResultTabRecord {
  tabKey: ResultTabKey;
  resultId: string;
  source: GraphOutputRefDto | null;
  title: string;
  presentation: ResultPresentation;
}

interface ResultWorkspaceState {
  order: ResultTabKey[];
  activeTabKey: ResultTabKey | null;
  tabs: Record<ResultTabKey, ResultTabRecord>;
}

interface ResultWorkspaceActions {
  openResult(descriptor: ResultDescriptor): ResultTabKey;
  setActiveTab(tabKey: ResultTabKey): void;
  closeTab(tabKey: ResultTabKey): void;
  moveTab(tabKey: ResultTabKey, targetTabKey: ResultTabKey): void;
  reset(): void;
}

export type ResultWorkspaceStore = ResultWorkspaceState & ResultWorkspaceActions;

const emptyState = (): ResultWorkspaceState => ({
  order: [],
  activeTabKey: null,
  tabs: {},
});

export function resultWorkspaceTabKey(descriptor: ResultDescriptor): ResultTabKey {
  const output = descriptor.provenance.output;
  return output
    ? `output:${graphOutputKey(output)}`
    : `result:${descriptor.resultId.length}:${descriptor.resultId}`;
}

export const useResultWorkspaceStore = create<ResultWorkspaceStore>((set) => ({
  ...emptyState(),

  openResult: (descriptor) => {
    const tabKey = resultWorkspaceTabKey(descriptor);
    const record: ResultTabRecord = {
      tabKey,
      resultId: descriptor.resultId,
      source: descriptor.provenance.output,
      title: descriptor.title,
      presentation: descriptor.presentation,
    };
    set((state) => ({
      order: state.tabs[tabKey] ? state.order : [...state.order, tabKey],
      tabs: { ...state.tabs, [tabKey]: record },
      activeTabKey: tabKey,
    }));
    return tabKey;
  },

  setActiveTab: (tabKey) => set((state) =>
    state.tabs[tabKey] ? { activeTabKey: tabKey } : {}),

  closeTab: (tabKey) => set((state) => {
    const closedIndex = state.order.indexOf(tabKey);
    if (closedIndex < 0) return {};
    const order = state.order.filter((key) => key !== tabKey);
    const tabs = { ...state.tabs };
    delete tabs[tabKey];
    const activeTabKey = state.activeTabKey === tabKey
      ? order[Math.min(closedIndex, order.length - 1)] ?? null
      : state.activeTabKey;
    return { order, tabs, activeTabKey };
  }),

  moveTab: (tabKey, targetTabKey) => set((state) => {
    const from = state.order.indexOf(tabKey);
    const to = state.order.indexOf(targetTabKey);
    if (from < 0 || to < 0 || from === to) return {};
    const order = [...state.order];
    const [moved] = order.splice(from, 1);
    order.splice(to, 0, moved);
    return { order };
  }),

  reset: () => set(emptyState()),
}));
