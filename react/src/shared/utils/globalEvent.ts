type GlobalEventTarget = Window | Document;

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
  listener: (event: Event) => void,
  options?: AddEventListenerOptions | boolean,
): () => void;
export function addGlobalEventListener(
  target: GlobalEventTarget,
  type: string,
  listener: EventListener,
  options?: AddEventListenerOptions | boolean,
) {
  target.addEventListener(type, listener, options);
  return () => target.removeEventListener(type, listener, options);
}
