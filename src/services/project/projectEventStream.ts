import { listen } from '@tauri-apps/api/event';
import { toErrorReference, type ErrorReference } from '@/services/ipc';
import {
  parseProjectEvent,
  type ProjectEvent,
  type ProjectEventParseCode,
} from './projectEventParser';

type UnlistenFn = () => void;

export type ProjectEventStreamItem =
  | { readonly kind: 'event'; readonly event: ProjectEvent }
  | { readonly kind: 'failure'; readonly issue: ErrorReference };

export type ProjectEventStreamStartOutcome =
  | { readonly ok: true; readonly value: undefined }
  | { readonly ok: false; readonly issue: ErrorReference };

export type ProjectEventListener = (item: ProjectEventStreamItem) => void;

export interface ProjectEventStream {
  start(): Promise<ProjectEventStreamStartOutcome>;
  close(): Promise<void>;
  subscribe(listener: ProjectEventListener): () => void;
}

const PROJECT_EVENT_NAME = 'project-event';
const SUBSCRIPTION_FAILURE_CODE = 'project_event_subscription_failed';
const CLOSED_CODE = 'project_event_stream_closed';

function parseFailureCode(code: ProjectEventParseCode): string {
  switch (code) {
    case 'invalidEnvelope':
      return 'project_event_invalid_envelope';
    case 'unknownType':
      return 'project_event_unknown_type';
    case 'invalidPayload':
      return 'project_event_invalid_payload';
  }
}

function closedOutcome(): ProjectEventStreamStartOutcome {
  return {
    ok: false,
    issue: { code: CLOSED_CODE, incidentId: null },
  };
}

function notifyListeners(
  listeners: ReadonlySet<ProjectEventListener>,
  item: ProjectEventStreamItem,
): void {
  for (const listener of [...listeners]) {
    try {
      listener(item);
    } catch {
      // A consumer callback cannot interrupt the raw listener or other consumers.
    }
  }
}

export function createProjectEventStream(): ProjectEventStream {
  const listeners = new Set<ProjectEventListener>();
  let closed = false;
  let unlisten: UnlistenFn | null = null;
  let startPromise: Promise<ProjectEventStreamStartOutcome> | null = null;
  let closePromise: Promise<void> | null = null;

  const handleEvent = (payload: unknown): void => {
    if (closed) return;
    const parsed = parseProjectEvent(payload);
    const item: ProjectEventStreamItem = parsed.ok
      ? { kind: 'event', event: parsed.event }
      : {
          kind: 'failure',
          issue: { code: parseFailureCode(parsed.code), incidentId: null },
        };
    notifyListeners(listeners, item);
  };

  const start = (): Promise<ProjectEventStreamStartOutcome> => {
    if (startPromise !== null) return startPromise;
    if (closed) return Promise.resolve(closedOutcome());

    const pending = listen<unknown>(PROJECT_EVENT_NAME, (event) => {
      handleEvent(event.payload);
    }).then(
      (cleanup): ProjectEventStreamStartOutcome => {
        if (closed) {
          cleanup();
          return closedOutcome();
        }
        unlisten = cleanup;
        return { ok: true, value: undefined };
      },
      (error): ProjectEventStreamStartOutcome => ({
        ok: false,
        issue: toErrorReference(error, SUBSCRIPTION_FAILURE_CODE),
      }),
    );
    startPromise = pending;
    return pending;
  };

  const close = (): Promise<void> => {
    if (closePromise !== null) return closePromise;
    closed = true;
    const pendingStart = startPromise;
    closePromise = (async () => {
      await pendingStart;
      const cleanup = unlisten;
      unlisten = null;
      cleanup?.();
      listeners.clear();
    })();
    return closePromise;
  };

  const subscribe = (listener: ProjectEventListener): (() => void) => {
    if (closed) return () => undefined;
    listeners.add(listener);
    let subscribed = true;
    return () => {
      if (!subscribed) return;
      subscribed = false;
      listeners.delete(listener);
    };
  };

  return { start, close, subscribe };
}
