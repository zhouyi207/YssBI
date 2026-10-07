import { resultReferenceKey, type ResultReference } from "@/shared/types/domain/result";
import { ResultContent } from "./ResultContent";

export function ResultPanel({ reference }: { readonly reference: ResultReference }) {
  return (
    <div
      className="flex h-full min-h-0 flex-col overflow-hidden bg-background"
      data-workbench-result-panel
    >
      <ResultContent key={resultReferenceKey(reference)} reference={reference} />
    </div>
  );
}
