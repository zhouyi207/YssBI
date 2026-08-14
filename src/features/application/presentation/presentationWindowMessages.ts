import type { PresentationWindowState } from './loadPresentationWindow';

export function presentationWindowErrorMessage(
  state: PresentationWindowState,
  messages: {
    missingResultId: string;
    notFound: string;
    loadFailed: string;
  },
): string | null {
  switch (state.status) {
    case 'missing_result_id':
      return messages.missingResultId;
    case 'not_found':
      return messages.notFound;
    case 'load_failed':
      return state.message || messages.loadFailed;
    case 'pending':
      return `Result pending (${state.progress.completed}/${state.progress.total ?? '?'})`;
    case 'failed':
      return state.failure.message;
    case 'cancelled':
      return 'Result was cancelled';
    default:
      return null;
  }
}
