import { ResultContent } from "./ResultContent";

export function ResultPanel({ resultId }: { readonly resultId: string }) {
  return (
    <div
      className="flex h-full min-h-0 flex-col overflow-hidden bg-background"
      data-workbench-result-panel
    >
      <ResultContent key={resultId} resultId={resultId} />
    </div>
  );
}
