import { bench, describe } from "vitest";
import { prepareSnapshotResources } from "@/features/application/editorMutation/projectSnapshotResources";
import { freezePublishedValue } from "@/shared/types/deepReadonly";
import { projectSnapshotFixture } from "@/tests/helpers/projectSnapshotFixtures";

for (const count of [100, 1000, 5000]) {
  describe(`${count} chart resources: unchanged snapshot preparation`, () => {
    const { current, plan } = projectSnapshotFixture(count);
    freezePublishedValue(current);
    bench(
      "share owned state",
      () => {
        prepareSnapshotResources(plan, current, []);
      },
      {
        time: 500,
        iterations: 20,
        warmupTime: 100,
      },
    );
    // Same production algorithm with one unnecessary ownership copy; not an old-version simulation.
    bench(
      "deep clone owned state before preparation",
      () => {
        prepareSnapshotResources(plan, structuredClone(current), []);
      },
      {
        time: 500,
        iterations: 20,
        warmupTime: 100,
      },
    );
  });
}
