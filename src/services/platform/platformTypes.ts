export type PlatformOperation =
  | 'openPathDialog'
  | 'savePathDialog'
  | 'showWindow'
  | 'setWindowTitle'
  | 'minimizeWindow'
  | 'toggleWindowMaximize'
  | 'readWindowMaximized'
  | 'closeWindow'
  | 'setWindowDecorations'
  | 'readWindowPosition'
  | 'readWindowSize'
  | 'readWindowScaleFactor'
  | 'subscribeWindowCloseRequested'
  | 'subscribeWindowResized'
  | 'createWebviewWindow'
  | 'readClipboardText'
  | 'writeClipboardText'
  | 'openExternal'
  | 'revealPath'
  | 'publishSettingsChanged'
  | 'subscribeSettingsChanged';

export interface PlatformFailureBase {
  readonly operation: PlatformOperation;
  readonly incidentId?: string;
}

export type PlatformFailure =
  | (PlatformFailureBase & { readonly code: 'unavailable' })
  | (PlatformFailureBase & {
      readonly code: 'permissionDenied';
      readonly capability: 'filesystem' | 'clipboard' | 'shell' | 'window' | 'event';
    })
  | (PlatformFailureBase & {
      readonly code: 'invalidArgument';
      readonly argument: 'options' | 'target' | 'windowLabel' | 'url' | 'geometry' | 'settings';
    })
  | (PlatformFailureBase & {
      readonly code: 'invalidResult';
      readonly resultKind: 'pathSelection' | 'windowGeometry' | 'windowState' | 'eventPayload';
    })
  | (PlatformFailureBase & { readonly code: 'operationFailed' });

export type PlatformOutcome<T> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly failure: PlatformFailure };

export type CloseRequestDecision = 'allow' | 'prevent';

export type PlatformUnsubscribe = () => void;
