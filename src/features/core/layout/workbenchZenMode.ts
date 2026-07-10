import { useLayoutStore } from './layoutStore';
import { setWorkbenchPartVisible } from './workbenchLayoutService';
import { WORKBENCH_PART_IDS, type WorkbenchPartId } from './workbenchLayoutDefaults';

type ZenPartSnapshot = {
  visible: boolean;
  userHidden?: boolean;
};

type ZenSnapshot = Record<WorkbenchPartId, ZenPartSnapshot>;

let zenSnapshot: ZenSnapshot | null = null;

function readPartSnapshot(partId: WorkbenchPartId): ZenPartSnapshot {
  const node = useLayoutStore.getState().nodes[partId];
  return {
    visible: node?.data?.visible !== false,
    userHidden: partId === 'detail' ? node?.data?.userHidden === true : undefined,
  };
}

export function isZenModeActive(): boolean {
  return useLayoutStore.getState().zenMode;
}

export function enterZenMode(): void {
  if (isZenModeActive()) return;

  zenSnapshot = {
    sidebar: readPartSnapshot('sidebar'),
    panel: readPartSnapshot('panel'),
    detail: readPartSnapshot('detail'),
  };

  for (const partId of WORKBENCH_PART_IDS) {
    setWorkbenchPartVisible(partId, false, { persist: false });
  }

  useLayoutStore.setState({ zenMode: true });
}

export function exitZenMode(): void {
  if (!isZenModeActive()) return;

  const saved = zenSnapshot;
  zenSnapshot = null;
  useLayoutStore.setState({ zenMode: false });

  if (!saved) return;

  for (const partId of WORKBENCH_PART_IDS) {
    const part = saved[partId];
    setWorkbenchPartVisible(partId, part.visible, {
      userHidden: partId === 'detail' ? part.userHidden : undefined,
      persist: false,
    });
  }
}

export function toggleZenMode(): void {
  if (isZenModeActive()) exitZenMode();
  else enterZenMode();
}
