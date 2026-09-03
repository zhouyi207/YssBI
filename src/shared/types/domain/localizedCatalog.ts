import {
  isNodeCreationDescriptorDto,
  type NodeCreationDescriptorDto,
} from "@/shared/types/domain/nodeCreationDescriptor";

export interface LocalizedCategoryDto {
  categoryId: string;
  parentCategoryId: string | null;
  order: number;
  title: string;
  searchText: string;
}

export type LocalizedPortDirectionDto = "input" | "output";
export type LocalizedPortKindDto = "data";

export interface LocalizedPortDto {
  key: string;
  label: string;
  direction: LocalizedPortDirectionDto;
  kind: LocalizedPortKindDto;
}

export interface LocalizedParameterDto {
  key: string;
  title: string;
  description: string | null;
}

export interface LocalizedCatalogItemDto {
  nodeTypeId: string;
  title: string;
  documentation: string | null;
  categoryId: string;
  iconId: string;
  styleId: string;
  aliases: string[];
  technicalTerms: string[];
  backendSearchText: string[];
  resourceNames: string[];
  ports: LocalizedPortDto[];
  parameters: LocalizedParameterDto[];
  resourcePath?: string;
  resourceRevision?: number;
  creation: NodeCreationDescriptorDto;
}

export interface LocalizedCatalogDto {
  projectInstanceId: string;
  registryFingerprint: string;
  resourcePublicationRevision: number;
  locale: string;
  categories: LocalizedCategoryDto[];
  items: LocalizedCatalogItemDto[];
}

function hasExactKeys(
  candidate: Record<string, unknown>,
  required: readonly string[],
  optional: readonly string[] = [],
): boolean {
  const allowed = new Set([...required, ...optional]);
  return (
    required.every((key) => Object.prototype.hasOwnProperty.call(candidate, key)) &&
    Object.keys(candidate).every((key) => allowed.has(key))
  );
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === "string";
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((entry) => typeof entry === "string");
}

function isLocalizedCategory(value: unknown): value is LocalizedCategoryDto {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  return (
    hasExactKeys(candidate, ["categoryId", "parentCategoryId", "order", "title", "searchText"]) &&
    typeof candidate.categoryId === "string" &&
    (candidate.parentCategoryId === null || typeof candidate.parentCategoryId === "string") &&
    Number.isSafeInteger(candidate.order) &&
    typeof candidate.title === "string" &&
    typeof candidate.searchText === "string"
  );
}

function isLocalizedPort(value: unknown): value is LocalizedPortDto {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  return (
    hasExactKeys(candidate, ["key", "label", "direction", "kind"]) &&
    typeof candidate.key === "string" &&
    typeof candidate.label === "string" &&
    (candidate.direction === "input" || candidate.direction === "output") &&
    candidate.kind === "data"
  );
}

function isLocalizedParameter(value: unknown): value is LocalizedParameterDto {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  return (
    hasExactKeys(candidate, ["key", "title", "description"]) &&
    typeof candidate.key === "string" &&
    typeof candidate.title === "string" &&
    isNullableString(candidate.description)
  );
}

export function isLocalizedCatalogItemDto(value: unknown): value is LocalizedCatalogItemDto {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  if (
    !hasExactKeys(
      candidate,
      [
        "nodeTypeId",
        "title",
        "documentation",
        "categoryId",
        "iconId",
        "styleId",
        "aliases",
        "technicalTerms",
        "backendSearchText",
        "resourceNames",
        "ports",
        "parameters",
        "creation",
      ],
      ["resourcePath", "resourceRevision"],
    )
  )
    return false;
  if (!isNodeCreationDescriptorDto(candidate.creation)) return false;
  const creation = candidate.creation;
  const coherent =
    creation.kind === "resourceBound"
      ? creation.nodeTypeId === candidate.nodeTypeId &&
        creation.resourcePath === candidate.resourcePath &&
        creation.resourceRevision === candidate.resourceRevision
      : candidate.resourcePath === undefined &&
        candidate.resourceRevision === undefined &&
        creation.nodeTypeId === candidate.nodeTypeId;
  return (
    coherent &&
    typeof candidate.nodeTypeId === "string" &&
    typeof candidate.title === "string" &&
    isNullableString(candidate.documentation) &&
    typeof candidate.categoryId === "string" &&
    typeof candidate.iconId === "string" &&
    typeof candidate.styleId === "string" &&
    isStringArray(candidate.aliases) &&
    isStringArray(candidate.technicalTerms) &&
    isStringArray(candidate.backendSearchText) &&
    isStringArray(candidate.resourceNames) &&
    Array.isArray(candidate.ports) &&
    candidate.ports.every(isLocalizedPort) &&
    Array.isArray(candidate.parameters) &&
    candidate.parameters.every(isLocalizedParameter) &&
    (candidate.resourcePath === undefined || typeof candidate.resourcePath === "string") &&
    (candidate.resourceRevision === undefined ||
      (Number.isSafeInteger(candidate.resourceRevision) &&
        (candidate.resourceRevision as number) >= 0))
  );
}

export function isLocalizedCatalogDto(value: unknown): value is LocalizedCatalogDto {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const candidate = value as Record<string, unknown>;
  return (
    hasExactKeys(candidate, [
      "projectInstanceId",
      "registryFingerprint",
      "resourcePublicationRevision",
      "locale",
      "categories",
      "items",
    ]) &&
    typeof candidate.projectInstanceId === "string" &&
    typeof candidate.registryFingerprint === "string" &&
    /^[0-9a-f]{64}$/.test(candidate.registryFingerprint) &&
    Number.isSafeInteger(candidate.resourcePublicationRevision) &&
    (candidate.resourcePublicationRevision as number) >= 0 &&
    typeof candidate.locale === "string" &&
    Array.isArray(candidate.categories) &&
    candidate.categories.every(isLocalizedCategory) &&
    Array.isArray(candidate.items) &&
    candidate.items.every(isLocalizedCatalogItemDto)
  );
}
