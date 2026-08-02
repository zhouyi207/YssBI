import {
  isNodeCreationDescriptor,
  type NodeCreationDescriptor,
} from './creationDescriptor';

export interface LocalizedCatalogCategory {
  categoryId: string;
  title: string;
  searchText: string;
}

export interface LocalizedCatalogItem {
  nodeTypeId: string;
  title: string;
  description: string | null;
  documentation: string | null;
  categoryId: string;
  aliases: string[];
  technicalTerms: string[];
  pinyin?: string;
  creation: NodeCreationDescriptor;
  searchText: string;
}

function isNullableString(value: unknown): value is string | null {
  return value === null || typeof value === 'string';
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((entry) => typeof entry === 'string');
}

export function isLocalizedCatalogItem(value: unknown): value is LocalizedCatalogItem {
  if (typeof value !== 'object' || value === null) return false;

  const candidate = value as Record<string, unknown>;
  return (
    typeof candidate.nodeTypeId === 'string' &&
    typeof candidate.title === 'string' &&
    isNullableString(candidate.description) &&
    isNullableString(candidate.documentation) &&
    typeof candidate.categoryId === 'string' &&
    isStringArray(candidate.aliases) &&
    isStringArray(candidate.technicalTerms) &&
    (candidate.pinyin === undefined || typeof candidate.pinyin === 'string') &&
    isNodeCreationDescriptor(candidate.creation) &&
    typeof candidate.searchText === 'string'
  );
}
