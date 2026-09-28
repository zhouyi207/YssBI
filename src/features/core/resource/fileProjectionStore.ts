import { create } from "zustand";
import type { FileSnapshot } from "@/shared/types/domain/fileDocument";
import type { FileResourceKind } from "@/shared/types/domain/resource";
export function createFileProjectionStore<S extends FileSnapshot<FileResourceKind, unknown>>() {
  const reads = new Map<string, object>();
  function beginRead(path: string): () => boolean {
    const token = {};
    reads.set(path, token);
    return () => reads.get(path) === token;
  }

  /** Read projections only; current authored documents belong to Rust Project. */
  const store = create<{
    documents: Record<string, S>;
    install(snapshot: S): boolean;
    remove(path: string): void;
    clear(): void;
  }>((set, get) => ({
    documents: {},
    install(snapshot) {
      const previous = get().documents[snapshot.path];
      if (
        previous?.projectInstanceId === snapshot.projectInstanceId &&
        previous.version.sessionId === snapshot.version.sessionId &&
        previous.version.revision > snapshot.version.revision
      )
        return false;
      reads.delete(snapshot.path);
      set((state) => ({ documents: { ...state.documents, [snapshot.path]: snapshot } }));
      return true;
    },
    remove(path) {
      reads.delete(path);
      set((state) => {
        const documents = { ...state.documents };
        delete documents[path];
        return { documents };
      });
    },
    clear() {
      reads.clear();
      set({ documents: {} });
    },
  }));

  return { store, beginRead };
}
