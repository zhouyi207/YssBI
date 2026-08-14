/** Result lifetime is owned by the project session; closing a window does not release it. */
export function usePresentationWindowLifecycle(_resultId: string | null | undefined): void {}
