import { useEditorCollections } from "@/features/core/editor";

/** Resource projection required by the polymorphic Details panel. */
export function useDetailResourceProjection() {
  return useEditorCollections();
}
