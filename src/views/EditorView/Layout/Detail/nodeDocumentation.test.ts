import { describe, expect, it } from 'vitest';
import type { NodeMetaData } from '@/shared/types/domain/node';
import { resolveNodeDocumentationContent } from './nodeDocumentation';

const meta: NodeMetaData = {
  uiStyle: 'default',
  supports_dynamic_pins: false,
  graph_scope: 'any',
  shell_role: null,
  documentation: {
    zh: '# 中文文档\n\n$\\hat{\\beta} = (X\'X)^{-1}X\'Y$',
    en: '# English doc\n\n$\\hat{\\beta} = (X\'X)^{-1}X\'Y$',
  },
};

describe('resolveNodeDocumentationContent', () => {
  it('returns localized documentation for the active language', () => {
    expect(resolveNodeDocumentationContent(meta, 'zh-CN', undefined)).toBe(
      '# 中文文档\n\n$\\hat{\\beta} = (X\'X)^{-1}X\'Y$',
    );
    expect(resolveNodeDocumentationContent(meta, 'en-US', undefined)).toBe(
      '# English doc\n\n$\\hat{\\beta} = (X\'X)^{-1}X\'Y$',
    );
  });

  it('falls back to the other language when primary is missing', () => {
    const partial: NodeMetaData = {
      uiStyle: 'default',
      supports_dynamic_pins: false,
      graph_scope: 'any',
      shell_role: null,
      documentation: { en: '# English only' },
    };
    expect(resolveNodeDocumentationContent(partial, 'zh-CN', undefined)).toBe('# English only');
  });

  it('falls back to instance description when documentation is missing', () => {
    expect(resolveNodeDocumentationContent(undefined, 'en', 'instance note')).toBe('instance note');
  });
});
