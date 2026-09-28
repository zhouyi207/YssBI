import type { FileVersion } from "@/shared/types/domain/fileDocument";
import { captureProjectLifecycleState } from "@/features/core/projectLifecycle/projectLifecycleAuthority";

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
  const source = inputs.get(activeKey(from));
  if (!source) return;
  const target = inputs.get(activeKey(to)) ?? new Map<string, DocumentInput>();
  for (const [id, input] of source) {
    input.remap?.(to, fromVersion, toVersion);
    target.set(id, input);
  }
  inputs.set(activeKey(to), target);
  inputs.delete(activeKey(from));
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
