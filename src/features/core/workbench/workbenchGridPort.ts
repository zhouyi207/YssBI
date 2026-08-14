import type { GridviewApi, SerializedGridviewComponent } from 'dockview-react';

interface Disposable { dispose(): void }

type PendingLayoutAction =
  | { type: 'restore'; layout: SerializedGridviewComponent }
  | { type: 'reset' };

function cloneLayout(layout: SerializedGridviewComponent): SerializedGridviewComponent {
  return structuredClone(layout);
}

export class WorkbenchGridPort {
  private api: GridviewApi | null = null;
  private defaultLayout: SerializedGridviewComponent | null = null;
  private disposables: Disposable[] = [];
  private pendingAction: PendingLayoutAction | null = null;
  private listeners = new Set<() => void>();

  bind(api: GridviewApi): void {
    this.unbind();
    this.api = api;
    this.defaultLayout = cloneLayout(api.toJSON());
    this.disposables = [api.onDidLayoutChange(() => this.emit())];
    const pendingAction = this.pendingAction;
    this.pendingAction = null;
    if (pendingAction?.type === 'restore') {
      api.fromJSON(cloneLayout(pendingAction.layout));
    } else if (pendingAction?.type === 'reset') {
      api.fromJSON(cloneLayout(this.defaultLayout));
    }
    this.emit();
  }

  unbind(api?: GridviewApi): void {
    if (!this.api || (api && api !== this.api)) return;
    this.disposables.forEach((item) => item.dispose());
    this.disposables = [];
    this.api = null;
    this.emit();
  }

  get isReady(): boolean { return this.api !== null; }
  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
  serialize(): SerializedGridviewComponent | null { return this.api?.toJSON() ?? null; }
  restore(layout: SerializedGridviewComponent): void {
    if (this.api) this.api.fromJSON(cloneLayout(layout));
    else this.pendingAction = { type: 'restore', layout: cloneLayout(layout) };
  }
  resetToDefault(): void {
    if (this.api && this.defaultLayout) this.api.fromJSON(cloneLayout(this.defaultLayout));
    else this.pendingAction = { type: 'reset' };
  }
  setPartVisible(id: string, visible: boolean): void { this.api?.getPanel(id)?.api.setVisible(visible); }
  setPartSize(id: string, size: number): void {
    const panel = this.api?.getPanel(id);
    if (!panel) return;
    panel.api.setSize(id === 'panel' ? { height: size } : { width: size });
  }
  movePart(
    id: string,
    direction: 'left' | 'right' | 'above' | 'below',
    referenceId: string,
    size?: number,
  ): void {
    const panel = this.api?.getPanel(id);
    const reference = this.api?.getPanel(referenceId);
    if (!panel || !reference) return;
    this.api?.movePanel(panel, { direction, reference: referenceId, size });
  }
  getPartVisible(id: string): boolean { return this.api?.getPanel(id)?.api.isVisible ?? false; }
  getPartSize(id: string): number | undefined {
    const panel = this.api?.getPanel(id);
    if (!panel) return undefined;
    return id === 'panel' ? panel.api.height : panel.api.width;
  }

  private emit(): void { this.listeners.forEach((listener) => listener()); }
}

export function createWorkbenchGridPort(): WorkbenchGridPort {
  return new WorkbenchGridPort();
}

export const workbenchGridPort = createWorkbenchGridPort();
