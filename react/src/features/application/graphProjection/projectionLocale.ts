import i18n from "i18next";
import { DEFAULT_LANGUAGE } from "@/shared/types/settings";

export function currentProjectionLocale(): string {
  return i18n.resolvedLanguage || i18n.language || DEFAULT_LANGUAGE;
}
