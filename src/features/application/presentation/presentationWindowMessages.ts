import type { PresentationWindowState } from './loadPresentationWindow';

export function presentationWindowErrorMessage(
  state: PresentationWindowState,
  messages: {
    missingResultId: string;
    notFound: string;
    loadFailed: string;
    pending: (completed: string, total: string | null) => string;
    executionFailed: string;
    upstreamFailed: string;
    cancelled: string;
  },
): string | null {
  switch (state.status) {
    case 'missing_result_id':
      return messages.missingResultId;
    case 'not_found':
      return messages.notFound;
    case 'load_failed':
      return messages.loadFailed;
    case 'pending':
      return messages.pending(state.progress.completed, state.progress.total);
    case 'failed':
      return state.failure.code === 'upstream_failed'
        ? messages.upstreamFailed
        : messages.executionFailed;
    case 'cancelled':
      return messages.cancelled;
    default:
      return null;
  }
}
