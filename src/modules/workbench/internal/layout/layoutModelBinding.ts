import { Actions, Model, type IJsonModel, type ModelChangeListener } from "flexlayout-react";

/** Holds the mounted native model; snapshots are notifications, never a second layout. */
export class LayoutModelBinding {
  private model: Model;
  private revision = 0;
  private snapshot: Readonly<{ model: Model; revision: number }>;
  private readonly listeners = new Set<() => void>();
  private readonly modelListener: ModelChangeListener = {
    onAfterAction: (action) => {
      const resizing =
        action.isAdjusting() &&
        (action.type === Actions.ADJUST_WEIGHTS || action.type === Actions.ADJUST_BORDER_SPLIT);
      // Native resize updates measured geometry directly. Notify application subscribers
      // on release; keep the revision current so speculative transactions still go stale.
      this.publish(!resizing);
    },
  };

  constructor(
    json: IJsonModel,
    private readonly configure?: (model: Model) => void,
  ) {
    this.model = Model.fromJson(structuredClone(json));
    this.configure?.(this.model);
    this.model.addChangeListener(this.modelListener);
    this.snapshot = { model: this.model, revision: this.revision };
  }
  getModel = (): Model => this.model;
  getSnapshot = () => this.snapshot;
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
  replace(json: IJsonModel): void {
    // Validate candidates before this commit: adoption transfers mounted tab content.
    const previous = this.model;
    const next = Model.fromJson(structuredClone(json), previous);
    this.configure?.(next);
    previous.removeChangeListener(this.modelListener);
    this.model = next;
    next.addChangeListener(this.modelListener);
    this.publish();
  }
  private publish(notify = true): void {
    this.snapshot = { model: this.model, revision: ++this.revision };
    if (!notify) return;
    for (const listener of Array.from(this.listeners)) {
      try {
        listener();
      } catch {
        /* Observers cannot interrupt a committed layout change. */
      }
    }
  }
}
