import { parseOlsReportSpec } from "@/shared/types/domain/olsReportSpec";
import { ResultProvider, Section, prepareSample } from "./sections";

export { prepareSample };
export const validate = parseOlsReportSpec;

export function Render({ spec, sample }) {
  return (
    <ResultProvider sample={sample}>
      {spec.sections
        .filter((section) => section.visible)
        .map((section) => (
          <Section key={section.id} kind={section.kind} />
        ))}
    </ResultProvider>
  );
}
