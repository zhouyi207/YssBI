import {
  IPC_MALFORMED_ERROR_CODE,
  IPC_TRANSPORT_FAILURE_CODE,
  IpcError,
  isIpcErrorCode,
} from '@/services/ipc';
import type { LifecycleMutationResultDto } from '@/shared/types/dto/project';
import { ProjectLifecycleProtocolError } from '@/features/application/projectLifecycleReceipt';

export type ProjectPickerPageOperation =
  | 'refresh'
  | 'scan'
  | 'cleanup'
  | 'open'
  | 'import'
  | 'remove'
  | 'favorite'
  | 'reveal';

export interface ProjectPickerErrorPresentation {
  code: string;
  incidentId: string | null;
  messageKey: string;
  fallbackMessageKey: string;
}

export interface ProjectPickerRecoveryPresentation {
  outcome: LifecycleMutationResultDto['outcome'];
  action: string;
  messageKey: string;
  path: string | null;
}

type ProjectPickerFailureIssueBase = {
  kind: 'failure';
  error: ProjectPickerErrorPresentation;
};

export type ProjectPickerPageIssue =
  | (ProjectPickerFailureIssueBase & {
      operation: 'refresh' | 'scan' | 'cleanup' | 'import';
    })
  | (ProjectPickerFailureIssueBase & {
      operation: 'open' | 'reveal';
      projectPath: string;
    })
  | (ProjectPickerFailureIssueBase & {
      operation: 'remove' | 'favorite';
      projectId: string;
    })
  | {
      kind: 'empty';
      operation: 'scan';
      reason: 'noneFound' | 'alreadyRegistered';
      found: number;
    }
  | {
      kind: 'empty';
      operation: 'cleanup';
      reason: 'noneFound';
    };

export type ProjectPickerPageActionOutcome =
  | { status: 'completed' }
  | { status: 'cancelled' }
  | { status: 'stale' }
  | { status: 'issue'; issue: ProjectPickerPageIssue };

export type ProjectPickerLifecycleActionOutcome =
  | { status: 'committed' }
  | { status: 'recovery'; recovery: ProjectPickerRecoveryPresentation }
  | { status: 'failed'; error: ProjectPickerErrorPresentation }
  | { status: 'stale' };

type ProjectPickerLocalErrorCode = 'project_activation_failed';

export class ProjectPickerOperationError extends Error {
  constructor(readonly code: ProjectPickerLocalErrorCode) {
    super(code);
    this.name = 'ProjectPickerOperationError';
  }
}

interface ErrorMessagePresentation {
  messageKey: string;
  fallbackMessageKey: string;
}

const DEFAULT_ERROR_PRESENTATION: ErrorMessagePresentation = {
  messageKey: 'projectPicker.issues.errors.unknown',
  fallbackMessageKey: 'common.error',
};

const ERROR_PRESENTATIONS: Readonly<Record<string, ErrorMessagePresentation>> = {
  invalid_path: {
    messageKey: 'projectPicker.issues.errors.invalidPath',
    fallbackMessageKey: 'projectPicker.newProjectModal.invalidPath',
  },
  invalid_project_root: {
    messageKey: 'projectPicker.issues.errors.invalidPath',
    fallbackMessageKey: 'projectPicker.newProjectModal.invalidPath',
  },
  project_not_found: {
    messageKey: 'projectPicker.issues.errors.projectNotFound',
    fallbackMessageKey: 'notifications.project.loadFailed',
  },
  invalid_project_format: {
    messageKey: 'projectPicker.issues.errors.invalidProject',
    fallbackMessageKey: 'notifications.project.loadFailed',
  },
  invalid_graph_document: {
    messageKey: 'projectPicker.issues.errors.invalidProject',
    fallbackMessageKey: 'notifications.project.loadFailed',
  },
  load_project_failed: {
    messageKey: 'projectPicker.issues.errors.loadFailed',
    fallbackMessageKey: 'notifications.project.loadFailed',
  },
  project_deserialize_failed: {
    messageKey: 'projectPicker.issues.errors.loadFailed',
    fallbackMessageKey: 'notifications.project.loadFailed',
  },
  project_io_failed: {
    messageKey: 'projectPicker.issues.errors.loadFailed',
    fallbackMessageKey: 'notifications.project.loadFailed',
  },
  project_activation_failed: {
    messageKey: 'projectPicker.issues.errors.loadFailed',
    fallbackMessageKey: 'notifications.project.loadFailed',
  },
  filesystem_transaction_busy: {
    messageKey: 'projectPicker.issues.errors.busy',
    fallbackMessageKey: 'common.error',
  },
  project_lifecycle_admission_closed: {
    messageKey: 'projectPicker.issues.errors.busy',
    fallbackMessageKey: 'common.error',
  },
  project_recovery_required: {
    messageKey: 'projectPicker.issues.errors.recoveryRequired',
    fallbackMessageKey: 'common.error',
  },
  transaction_prepare_failed: {
    messageKey: 'projectPicker.issues.errors.filesystem',
    fallbackMessageKey: 'common.error',
  },
  transaction_commit_failed: {
    messageKey: 'projectPicker.issues.errors.filesystem',
    fallbackMessageKey: 'common.error',
  },
  transaction_rollback_failed: {
    messageKey: 'projectPicker.issues.errors.filesystem',
    fallbackMessageKey: 'common.error',
  },
  [IPC_TRANSPORT_FAILURE_CODE]: {
    messageKey: 'projectPicker.issues.errors.transport',
    fallbackMessageKey: 'common.error',
  },
  [IPC_MALFORMED_ERROR_CODE]: {
    messageKey: 'projectPicker.issues.errors.unexpectedResponse',
    fallbackMessageKey: 'common.error',
  },
  internal_error: {
    messageKey: 'projectPicker.issues.errors.internal',
    fallbackMessageKey: 'common.error',
  },
  project_lifecycle_protocol_error: {
    messageKey: 'projectPicker.issues.errors.unexpectedResponse',
    fallbackMessageKey: 'common.error',
  },
};

const RECOVERY_MESSAGE_KEYS: Readonly<Record<string, string>> = {
  registerDestination: 'projectPicker.issues.recovery.registerDestination',
  activateDestination: 'projectPicker.issues.recovery.activateDestination',
  cleanupRegistry: 'projectPicker.issues.recovery.cleanupRegistry',
  removeRegistryRecord: 'projectPicker.issues.recovery.removeRegistryRecord',
};

export function projectPickerErrorPresentation(
  error: unknown,
): ProjectPickerErrorPresentation {
  let code = 'unknown_error';
  let incidentId: string | null = null;

  if (error instanceof IpcError) {
    code = error.code;
    incidentId = error.incidentId;
  } else if (error instanceof ProjectLifecycleProtocolError) {
    code = error.code;
  } else if (error instanceof ProjectPickerOperationError) {
    code = error.code;
  }

  const presentation = ERROR_PRESENTATIONS[code] ?? DEFAULT_ERROR_PRESENTATION;
  return { code, incidentId, ...presentation };
}

export function projectPickerRecoveryPresentation(
  result: LifecycleMutationResultDto,
): ProjectPickerRecoveryPresentation {
  const action = result.recovery?.action ?? result.outcome;
  return {
    outcome: result.outcome,
    action,
    messageKey: RECOVERY_MESSAGE_KEYS[action] ?? 'projectPicker.issues.recovery.unknown',
    path: result.recovery?.path ?? result.path,
  };
}

export function isProjectPickerStaleError(error: unknown): boolean {
  return isIpcErrorCode(error, 'stale_project_lifecycle');
}
