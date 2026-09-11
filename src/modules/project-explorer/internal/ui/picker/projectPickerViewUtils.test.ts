import { expect, it } from "vitest";
import { formatProjectStamp } from "./projectPickerViewUtils";

it("retains calendar clock fields without applying an input timezone", () => {
  expect(formatProjectStamp("2026-09-11T10:00:00+08:00")).toBe("2026-09-11 10:00");
  expect(formatProjectStamp("2026-09-11T10:00:00Z")).toBe("2026-09-11 10:00");
  expect(formatProjectStamp("2026-09-11T10:00:00")).toBe("2026-09-11 10:00");
  expect(formatProjectStamp("unknown")).toBe("unknown");
});
