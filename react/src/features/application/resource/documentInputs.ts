import type { FileVersion } from "@/shared/types/domain/fileDocument";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
  ProjectLifecycleError,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

/** Text composition belongs to the file, even while its field is unmounted. */
interface DocumentInput {
  dirty(): boolean;
  flush(): Promise<void>;
  prepareDiscard(): () => void;
  release?(): void;
  remap?(path: string, fromVersion?: FileVersion, toVersion?: FileVersion): void;
}
const inputs = new Map<string, Map<string, DocumentInput>>();
const activeKey = (path: string) => captureProjectLifecycleState().projectInstanceId + ":" + path;
function documentInputs(path: string) {
  return inputs.get(activeKey(path))?.values() ?? [];
}

export function retainDocumentInput<T extends DocumentInput>(
  project: string,
  path: string,
  inputId: string,
  create: () => T,
): T {
  const key = project + ":" + path;
  const entries = inputs.get(key) ?? new Map<string, DocumentInput>();
  const existing = entries.get(inputId);
  if (existing) return existing as T;
  const input = create();
  entries.set(inputId, input);
  inputs.set(key, entries);
  return input;
}

/** A committed rename transfers the same buffers, including unfinished text. */
export function remapDocumentInputs(
  from: string,
  to: string,
  fromVersion?: FileVersion,
  toVersion?: FileVersion,
): void {
  if (from === to) return;
  const owner = captureProjectLifecycleState();
  const sourceKey = owner.projectInstanceId + ":" + from;
  const targetKey = owner.projectInstanceId + ":" + to;
  const source = inputs.get(sourceKey);
  if (!source) return;
  const moving = [...source];
  const target = inputs.get(targetKey) ?? new Map<string, DocumentInput>();
  // Transfer registrations before buffer notifications can release or replace them.
  for (const [id, input] of moving) target.set(id, input);
  inputs.set(targetKey, target);
  inputs.delete(sourceKey);
  for (const [id, input] of moving) {
    if (!isProjectLifecycleStateCurrent(owner)) throw new ProjectLifecycleError();
    if (inputs.get(targetKey) !== target || target.get(id) !== input) continue;
    input.remap?.(to, fromVersion, toVersion);
    if (!isProjectLifecycleStateCurrent(owner)) throw new ProjectLifecycleError();
  }
}

export function releaseDocumentInputs(path: string): void {
  for (const input of documentInputs(path)) input.release?.();
  inputs.delete(activeKey(path));
}
export function resetDocumentInputs(): void {
  for (const entries of inputs.values()) for (const input of entries.values()) input.release?.();
  inputs.clear();
}
export function hasPendingDocumentInput(path: string): boolean {
  return [...documentInputs(path)].some((input) => input.dirty());
}
export async function flushDocumentInputs(path: string): Promise<void> {
  for (const input of documentInputs(path)) await input.flush();
}
export function prepareDocumentInputDiscard(path: string): () => void {
  const commits = [...documentInputs(path)].map((input) => input.prepareDiscard());
  return () => {
    for (const commit of commits) commit();
  };
}
