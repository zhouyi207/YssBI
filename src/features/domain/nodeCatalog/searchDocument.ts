import { pinyin } from "pinyin-pro";
import type { LocalizedCatalogItemDto } from "@/shared/types/domain/localizedCatalog";

const HAN_CHARACTER = /\p{Script=Han}/u;
const PINYIN_OPTIONS = {
  toneType: "none",
  type: "array",
  mode: "normal",
  nonZh: "consecutive",
} as const;

export function normalizeCatalogSearchText(value: string): string {
  return value
    .normalize("NFKD")
    .replace(/\p{Mark}/gu, "")
    .toLowerCase()
    .replace(/[^\p{Letter}\p{Number}]+/gu, " ")
    .trim();
}

function normalizedUnique(values: Iterable<string>): string[] {
  return [...new Set([...values].map(normalizeCatalogSearchText).filter(Boolean))];
}

function pinyinForms(values: readonly string[], pattern: "pinyin" | "first"): string[] {
  return values
    .filter((value) => HAN_CHARACTER.test(value))
    .map((value) =>
      pinyin(value, {
        ...PINYIN_OPTIONS,
        pattern,
      }).join(pattern === "first" ? "" : " "),
    );
}

export function buildCatalogSearchDocument(item: LocalizedCatalogItemDto): string {
  const sources = [
    item.title,
    ...item.aliases,
    ...item.technicalTerms,
    ...item.backendSearchText,
    ...item.resourceNames,
  ];

  return normalizedUnique([
    item.nodeTypeId,
    ...sources,
    ...pinyinForms(sources, "pinyin"),
    ...pinyinForms(sources, "first"),
  ]).join(" ");
}

export function catalogSearchTerms(query: string): string[] {
  return normalizeCatalogSearchText(query).split(" ").filter(Boolean);
}

export function matchesCatalogSearchDocument(document: string, terms: readonly string[]): boolean {
  return terms.every((term) => document.includes(term));
}
