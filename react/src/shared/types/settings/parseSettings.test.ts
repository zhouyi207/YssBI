import { expect, it } from "vitest";
import { parseSettingsPatch } from "./parseSettings";

it("distinguishes absent settings from malformed fields at both input boundaries", () => {
  expect(parseSettingsPatch({})).toEqual({});
  expect(parseSettingsPatch({ appearance: { smoothScroll: false, language: "en-US" } })).toEqual({
    appearance: { smoothScroll: false, language: "en-US" },
  });
  for (const payload of [
    null,
    [],
    { appearance: null },
    { appearance: [] },
    { appearance: { smoothScroll: "false" } },
    { appearance: { language: "invalid" } },
    { appearance: { titleBarStyle: "invalid" } },
    { appearance: { colorTheme: 42 } },
  ])
    expect(parseSettingsPatch(payload)).toBeNull();
});
