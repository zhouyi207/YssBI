import { defineCatalog } from "@json-render/core";
import { schema } from "@json-render/react/schema";
import {
  defineRegistry,
  Renderer,
  StateProvider,
  VisibilityProvider,
  ActionProvider,
} from "@json-render/react";
import { z } from "zod";
import { parseOlsReportSpec } from "@/shared/types/domain/olsReportSpec";
import { ResultProvider, Section, prepareSample } from "./sections";

export { prepareSample };
const kinds = ["modelSummary", "coefficientTable", "residualPlot"];
const catalog = defineCatalog(schema, {
  components: {
    Report: { props: z.object({}).strict(), slots: ["default"] },
    ...Object.fromEntries(kinds.map((kind) => [kind, { props: z.object({}).strict() }])),
  },
  actions: {},
});
const { registry } = defineRegistry(catalog, {
  components: {
    Report: ({ children }) => <>{children}</>,
    ...Object.fromEntries(kinds.map((kind) => [kind, () => <Section kind={kind} />])),
  },
});

export function validate(raw, source) {
  // Library schemas do not authorize Result references, bound section counts or impose our flat layout.
  const parsed = parseOlsReportSpec(raw, source);
  if (!parsed.ok) return parsed;
  const visible = parsed.value.sections.filter((section) => section.visible);
  const spec = {
    root: "$report",
    elements: {
      $report: { type: "Report", props: {}, children: visible.map((section) => section.id) },
      ...Object.fromEntries(
        visible.map((section) => [section.id, { type: section.kind, props: {}, children: [] }]),
      ),
    },
  };
  const validation = catalog.validate(spec);
  return validation.success ? { ok: true, value: spec } : { ok: false, issue: validation.error };
}

export function Render({ spec, sample }) {
  return (
    <ResultProvider sample={sample}>
      <StateProvider>
        <VisibilityProvider>
          <ActionProvider handlers={{}}>
            <Renderer spec={spec} registry={registry} />
          </ActionProvider>
        </VisibilityProvider>
      </StateProvider>
    </ResultProvider>
  );
}
