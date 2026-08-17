import { isIpcErrorDto, type IpcErrorDto } from '@/shared/types/dto/ipcError';

export const IPC_TRANSPORT_FAILURE_CODE = 'ipc_transport_failure';
export const IPC_MALFORMED_ERROR_CODE = 'ipc_malformed_error';

export type IpcErrorKind = 'backend' | 'transport' | 'malformed';

export interface IpcErrorInit {
  kind: IpcErrorKind;
  command: string;
  code: string;
  details: IpcErrorDto['details'];
  incidentId: string | null;
  cause: unknown;
}

export interface ErrorReference {
  code: string;
  incidentId: string | null;
}

function errorMessage(init: IpcErrorInit): string {
  if (init.kind === 'backend') {
    return `IPC command '${init.command}' failed with code '${init.code}'`;
  }
  if (init.kind === 'transport') {
    const detail = init.cause instanceof Error ? `: ${init.cause.message}` : '';
    return `IPC transport failed for command '${init.command}'${detail}`;
  }
  return `IPC command '${init.command}' rejected with a malformed error payload`;
}

export class IpcError extends Error {
  readonly kind: IpcErrorKind;
  readonly command: string;
  readonly code: string;
  readonly details: IpcErrorDto['details'];
  readonly incidentId: string | null;
  declare readonly cause: unknown;

  constructor(init: IpcErrorInit) {
    super(errorMessage(init));
    this.name = 'IpcError';
    this.kind = init.kind;
    this.command = init.command;
    this.code = init.code;
    this.details = init.details;
    this.incidentId = init.incidentId;
    Object.defineProperty(this, 'cause', {
      configurable: true,
      value: init.cause,
    });
  }
}

export function normalizeIpcError(command: string, error: unknown): IpcError {
  if (error instanceof IpcError) return error;
  if (error instanceof Error) {
    return new IpcError({
      kind: 'transport',
      command,
      code: IPC_TRANSPORT_FAILURE_CODE,
      details: null,
      incidentId: null,
      cause: error,
    });
  }
  if (isIpcErrorDto(error)) {
    return new IpcError({
      kind: 'backend',
      command,
      code: error.code,
      details: error.details,
      incidentId: error.incidentId,
      cause: error,
    });
  }
  return new IpcError({
    kind: 'malformed',
    command,
    code: IPC_MALFORMED_ERROR_CODE,
    details: null,
    incidentId: null,
    cause: error,
  });
}

export function toErrorReference(error: unknown, fallbackCode: string): ErrorReference {
  return error instanceof IpcError
    ? { code: error.code, incidentId: error.incidentId }
    : { code: fallbackCode, incidentId: null };
}

export function isIpcError(value: unknown): value is IpcError {
  return value instanceof IpcError;
}

export function isIpcErrorCode<Code extends string>(
  value: unknown,
  code: Code,
): value is IpcError & { readonly code: Code } {
  return value instanceof IpcError && value.code === code;
}
