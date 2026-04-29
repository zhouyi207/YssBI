type GlobalEventTarget = Window | Document;

function withAbortSignal(options: AddEventListenerOptions | boolean | undefined, signal: AbortSignal) {
  if (typeof options === "boolean") {
    return { capture: options, signal };
  }
  return { ...(options ?? {}), signal };
}

export function addGlobalEventListener<K extends keyof WindowEventMap>(
  target: Window,
  type: K,
  listener: (event: WindowEventMap[K]) => void,
  options?: AddEventListenerOptions | boolean,
): () => void;
export function addGlobalEventListener<K extends keyof DocumentEventMap>(
  target: Document,
  type: K,
  listener: (event: DocumentEventMap[K]) => void,
  options?: AddEventListenerOptions | boolean,
): () => void;
export function addGlobalEventListener(
  target: GlobalEventTarget,
  type: string,
  listener: EventListener,
  options?: AddEventListenerOptions | boolean,
) {
  if (typeof AbortController === "undefined") {
    target.addEventListener(type, listener, options);
    return () => target.removeEventListener(type, listener, options);
  }

  const controller = new AbortController();
  target.addEventListener(type, listener, withAbortSignal(options, controller.signal));
  return () => controller.abort();
}
