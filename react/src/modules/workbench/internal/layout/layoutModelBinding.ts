import {
  GroupAction,
  Model,
  type Action,
  type IJsonModel,
  type ModelChangeListener,
} from "flexlayout-react";

/** Intermediate native actions advance mutation identity; the final action commits notification. */
export function isIntermediateLayoutAction(action: Action): boolean {
  return (
    action.isAdjusting() ||
    (action instanceof GroupAction &&
      action.actions.length > 0 &&
      action.actions.every(isIntermediateLayoutAction))
  );
}

/** Holds the mounted native model; snapshots are notifications, never a second layout. */
export class LayoutModelBinding {
  private model: Model;
  private revision = 0;
  private snapshot: Readonly<{ model: Model; revision: number }>;
  private readonly listeners = new Set<() => void>();
  private readonly modelListeners = new Set<() => void>();
  private readonly modelListener: ModelChangeListener = {
    onAfterAction: (action) => {
      // Geometry gestures and host corrections keep native rendering current. Only the final
      // action notifies application subscribers; every action still invalidates old candidates.
      this.publish(!isIntermediateLayoutAction(action));
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
  subscribeModel = (listener: () => void): (() => void) => {
    this.modelListeners.add(listener);
    return () => this.modelListeners.delete(listener);
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
    this.notify(this.modelListeners);
  }
  private publish(notify = true): void {
    this.snapshot = { model: this.model, revision: ++this.revision };
    if (!notify) return;
    this.notify(this.listeners);
  }
  private notify(listeners: ReadonlySet<() => void>): void {
    for (const listener of Array.from(listeners)) {
      try {
        listener();
      } catch {
        /* Observers cannot interrupt a committed layout change. */
      }
    }
  }
}
