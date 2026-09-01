import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  CloseRequestDecision,
  PlatformFailure,
  PlatformOutcome,
  PlatformUnsubscribe,
} from "./platformTypes";

export interface AppWindowHandle {
  readonly label: string;
  show(): Promise<PlatformOutcome<void>>;
  setTitle(title: string): Promise<PlatformOutcome<void>>;
  minimize(): Promise<PlatformOutcome<void>>;
  toggleMaximize(): Promise<PlatformOutcome<void>>;
  isMaximized(): Promise<PlatformOutcome<boolean>>;
  close(): Promise<PlatformOutcome<void>>;
  setDecorations(enabled: boolean): Promise<PlatformOutcome<void>>;
  outerPosition(): Promise<PlatformOutcome<Readonly<{ x: number; y: number }>>>;
  innerSize(): Promise<PlatformOutcome<Readonly<{ width: number; height: number }>>>;
  scaleFactor(): Promise<PlatformOutcome<number>>;
  onCloseRequested(
    listener: () => CloseRequestDecision | Promise<CloseRequestDecision>,
  ): Promise<PlatformOutcome<PlatformUnsubscribe>>;
  onResized(listener: () => void): Promise<PlatformOutcome<PlatformUnsubscribe>>;
}

function operationFailure(operation: PlatformFailure["operation"]): PlatformFailure {
  return { operation, code: "operationFailed" };
}

function invalidTitle(): PlatformFailure {
  return { operation: "setWindowTitle", code: "invalidArgument", argument: "options" };
}

function call<T>(
  operation: PlatformFailure["operation"],
  action: () => Promise<T>,
): Promise<PlatformOutcome<T>> {
  return action()
    .then((value) => ({ ok: true, value }) as const)
    .catch(() => ({ ok: false, failure: operationFailure(operation) }) as const);
}

export function currentAppWindow(): AppWindowHandle {
  const native = getCurrentWindow();
  return {
    label: native.label,
    show: () => call("showWindow", () => native.show()),
    setTitle: (title) =>
      title.trim().length === 0
        ? Promise.resolve({ ok: false, failure: invalidTitle() })
        : call("setWindowTitle", () => native.setTitle(title)),
    minimize: () => call("minimizeWindow", () => native.minimize()),
    toggleMaximize: () => call("toggleWindowMaximize", () => native.toggleMaximize()),
    isMaximized: () => call("readWindowMaximized", () => native.isMaximized()),
    close: () => call("closeWindow", () => native.close()),
    setDecorations: (enabled) => call("setWindowDecorations", () => native.setDecorations(enabled)),
    outerPosition: async () => {
      const result = await call("readWindowPosition", () => native.outerPosition());
      return result.ok ? { ok: true, value: { x: result.value.x, y: result.value.y } } : result;
    },
    innerSize: async () => {
      const result = await call("readWindowSize", () => native.innerSize());
      return result.ok
        ? { ok: true, value: { width: result.value.width, height: result.value.height } }
        : result;
    },
    scaleFactor: () => call("readWindowScaleFactor", () => native.scaleFactor()),
    onCloseRequested: (listener) =>
      call("subscribeWindowCloseRequested", () =>
        native.onCloseRequested(async (event) => {
          if ((await listener()) === "prevent") event.preventDefault();
        }),
      ),
    onResized: (listener) =>
      call("subscribeWindowResized", () => native.onResized(() => listener())),
  };
}
