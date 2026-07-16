import { getNodeDefinitionMeta, pickLocalizedText, type NodeDefinition } from '@/shared/types/domain';

export interface NodeDocumentationSearchResult {
  nodeType: string;
  name: string;
  category: string[];
  description?: string;
  documentation?: string;
}

function textIncludes(text: string | undefined, query: string): boolean {
  return text?.toLocaleLowerCase().includes(query) ?? false;
}

interface SearchableNodeDocumentationResult extends NodeDocumentationSearchResult {
  searchableDocumentation: string;
}

function relevance(result: SearchableNodeDocumentationResult, query: string): number {
  if (textIncludes(result.name, query)) return 5;
  if (textIncludes(result.nodeType, query)) return 4;
  if (result.category.some((entry) => textIncludes(entry, query))) return 3;
  if (textIncludes(result.description, query)) return 2;
  return textIncludes(result.searchableDocumentation, query) ? 1 : 0;
}

/**
 * Searches every user-facing node field and every available documentation language.
 * The result content still follows the current UI language with a language fallback.
 */
export function searchNodeDocumentation(
  definitions: NodeDefinition[],
  query: string,
  language: string,
): NodeDocumentationSearchResult[] {
  const results: SearchableNodeDocumentationResult[] = definitions.map((definition) => {
    const meta = getNodeDefinitionMeta(definition);
    return {
      nodeType: definition.nodeType,
      name: definition.name,
      category: definition.category ?? [],
      description: meta?.description,
      documentation: pickLocalizedText(meta?.documentation, language),
      searchableDocumentation: Object.values(meta?.documentation ?? {}).join('\n'),
    };
  });
  const normalizedQuery = query.trim().toLocaleLowerCase();

  return results
    .filter((result) => !normalizedQuery || relevance(result, normalizedQuery) > 0)
    .sort((a, b) => {
      const relevanceDelta = relevance(b, normalizedQuery) - relevance(a, normalizedQuery);
      return relevanceDelta || a.name.localeCompare(b.name);
    })
    .map(({ searchableDocumentation: _searchableDocumentation, ...result }) => result);
}
