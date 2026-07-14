import { useLayoutStore } from './layoutStore';
import { WORKBENCH_PART_IDS, type WorkbenchPartId } from './workbenchLayoutDefaults';
import { commitEditorGridLayoutState } from './editorGridSizing';

type ZenPartSnapshot = {
  visible: boolean;
  userHidden?: boolean;
};

type ZenSnapshot = Record<WorkbenchPartId, ZenPartSnapshot>;

function readPartSnapshot(partId: WorkbenchPartId): ZenPartSnapshot {
  const node = useLayoutStore.getState().nodes[partId];
  return {
    visible: node?.data?.visible !== false,
    userHidden: node?.data?.userHidden === true,
  };
}

function setPartSnapshot(partId: WorkbenchPartId, snapshot: ZenPartSnapshot): void {
  const node = useLayoutStore.getState().nodes[partId];
  if (!node) return;

  const data = { ...node.data, visible: snapshot.visible, userHidden: snapshot.userHidden ?? false };
  useLayoutStore.getState().updateNode(partId, { data });
}

class ZenModeSessionController {
  private snapshot: ZenSnapshot | null = null;

  enter(): void {
    if (useLayoutStore.getState().zenMode) return;

    this.snapshot = {
      sidebar: readPartSnapshot('sidebar'),
      panel: readPartSnapshot('panel'),
      detail: readPartSnapshot('detail'),
    };

    for (const partId of WORKBENCH_PART_IDS) {
      setPartSnapshot(partId, { visible: false });
    }
    useLayoutStore.setState((state) => {
      commitEditorGridLayoutState(state.nodes);
      state.zenMode = true;
    });
  }

  exit(): void {
    if (!useLayoutStore.getState().zenMode) return;

    const saved = this.snapshot;
    this.snapshot = null;
    useLayoutStore.setState({ zenMode: false });

    if (!saved) return;
    for (const partId of WORKBENCH_PART_IDS) {
      setPartSnapshot(partId, saved[partId]);
    }
    useLayoutStore.setState((state) => {
      commitEditorGridLayoutState(state.nodes);
    });
  }

  clear(): void {
    this.snapshot = null;
    useLayoutStore.setState({ zenMode: false });
  }
}

const zenModeSession = new ZenModeSessionController();

export function isZenModeActive(): boolean {
  return useLayoutStore.getState().zenMode;
}

export function enterZenMode(): void {
  zenModeSession.enter();
}

export function exitZenMode(): void {
  zenModeSession.exit();
}

/** Drop all session-only Zen state without restoring its visibility snapshot. */
export function clearZenModeSession(): void {
  zenModeSession.clear();
}

export function toggleZenMode(): void {
  if (isZenModeActive()) exitZenMode();
  else enterZenMode();
}
