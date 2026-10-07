import { createContext, useContext, type ReactNode } from "react";

export type ResultViewPresentation = "standalone" | "embedded";

const ResultViewPresentationContext = createContext<ResultViewPresentation>("standalone");

export function ResultViewPresentationProvider({
  presentation,
  children,
}: {
  presentation: ResultViewPresentation;
  children: ReactNode;
}) {
  return (
    <ResultViewPresentationContext.Provider value={presentation}>
      {children}
    </ResultViewPresentationContext.Provider>
  );
}

export function useResultViewPresentation(): ResultViewPresentation {
  return useContext(ResultViewPresentationContext);
}
