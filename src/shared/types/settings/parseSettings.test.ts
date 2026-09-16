import { expect, it } from "vitest";
import { parseSettingsPatch } from "./parseSettings";

it("distinguishes absent settings from malformed fields at both input boundaries", () => {
  expect(parseSettingsPatch({})).toEqual({});
  expect(parseSettingsPatch({ ai: { openAiModel: "model" } })).toEqual({
    ai: { openAiModel: "model" },
  });
  expect(parseSettingsPatch({ appearance: { smoothScroll: false, language: "en-US" } })).toEqual({
    appearance: { smoothScroll: false, language: "en-US" },
  });
  for (const payload of [
    null,
    [],
    { ai: null },
    { ai: [] },
    { ai: { openAiApiKey: 123 }, appearance: {} },
    { ai: { openAiModel: undefined } },
    { appearance: { smoothScroll: "false" } },
    { appearance: { language: "invalid" } },
    { appearance: { titleBarStyle: "invalid" } },
    { appearance: { colorTheme: 42 } },
  ])
    expect(parseSettingsPatch(payload)).toBeNull();
});
