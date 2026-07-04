import type { PresentationWindowState } from './loadPresentationWindow';

export function presentationWindowErrorMessage(
  state: PresentationWindowState,
  messages: {
    missingSourceId: string;
    notFound: string;
    loadFailed: string;
  },
): string | null {
  switch (state.status) {
    case 'missing_source_id':
      return messages.missingSourceId;
    case 'not_found':
      return messages.notFound;
    case 'load_failed':
      return state.message || messages.loadFailed;
    default:
      return null;
  }
}
