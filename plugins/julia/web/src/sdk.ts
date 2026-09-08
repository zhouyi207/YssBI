export interface HostContext {
  language: string;
  viewId: string;
  theme: Record<string, string>;
  themeMode?: "light" | "dark";
  visible?: boolean;
}
let current: HostContext = { language: "zh-CN", viewId: "runtime", theme: {} };
let port: MessagePort | undefined;
let nextId = 0;
const waiting = new Map<
  string,
  {
    resolve(value: unknown): void;
    reject(value: unknown): void;
    timer: ReturnType<typeof setTimeout>;
  }
>();
const listeners = new Set<() => void>();
let resolveReady: () => void;
export const ready = new Promise<void>((resolve) => {
  resolveReady = resolve;
});
function receive(event: MessageEvent) {
  if (
    event.source !== window.parent ||
    event.data?.type !== "yssbi:plugin-init" ||
    !event.ports[0] ||
    port
  )
    return;
  port = event.ports[0];
  current = event.data;
  window.removeEventListener("message", receive);
  port.onmessage = (event) => {
    if (event.data?.type === "context") {
      current = event.data;
      listeners.forEach((listener) => listener());
      return;
    }
    const message = event.data;
    const pending = waiting.get(message?.id);
    if (!pending) return;
    waiting.delete(message.id);
    clearTimeout(pending.timer);
    if (message.error) pending.reject(message.error);
    else pending.resolve(message.result);
  };
  resolveReady();
  listeners.forEach((listener) => listener());
}
window.addEventListener("message", receive);
export function context() {
  return current;
}
export function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
export async function request<T = unknown>(method: string, input: unknown = null): Promise<T> {
  await ready;
  if (waiting.size >= 16)
    throw { code: "plugin_resource_exhausted", details: null, incidentId: null };
  const id = String(++nextId);
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      waiting.delete(id);
      reject({ code: "plugin_request_timeout", details: null, incidentId: null });
    }, 35_000);
    waiting.set(id, { resolve: resolve as (value: unknown) => void, reject, timer });
    port!.postMessage({ id, method, input });
  });
}
