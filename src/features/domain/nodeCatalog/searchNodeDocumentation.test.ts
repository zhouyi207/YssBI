import { describe, expect, it } from 'vitest';
import type { NodeDefinition } from '@/shared/types/domain';
import { searchNodeDocumentation } from './searchNodeDocumentation';

const definitions: NodeDefinition[] = [
  {
    name: 'Linear Regression',
    category: ['Statistics', 'Regression'],
    nodeType: 'statistics.linear_regression',
    nodeMetadata: {
      uiStyle: 'default',
      description: 'Fits an ordinary least squares model.',
      documentation: {
        en: 'Use this node to estimate coefficients and inspect residuals.',
        zh: '使用此节点估计系数并检查残差。',
      },
      supports_dynamic_pins: false,
      graph_scope: 'any',
      shell_role: null,
    },
    pinSlots: [],
    typeCapabilities: [],
  },
  {
    name: 'Filter Rows',
    category: ['Data'],
    nodeType: 'data.filter_rows',
    nodeMetadata: {
      uiStyle: 'default',
      description: 'Removes rows that do not match a condition.',
      supports_dynamic_pins: false,
      graph_scope: 'any',
      shell_role: null,
    },
    pinSlots: [],
    typeCapabilities: [],
  },
];

describe('searchNodeDocumentation', () => {
  it('matches every available documentation language while preserving the UI language for display', () => {
    const englishSearch = searchNodeDocumentation(definitions, 'residuals', 'zh-CN');
    const chineseSearch = searchNodeDocumentation(definitions, '残差', 'en-US');

    expect(englishSearch).toEqual([expect.objectContaining({ nodeType: 'statistics.linear_regression' })]);
    expect(chineseSearch).toEqual([expect.objectContaining({ nodeType: 'statistics.linear_regression' })]);
    expect(englishSearch[0]?.documentation).toBe('使用此节点估计系数并检查残差。');
  });

  it('matches node descriptions and ranks title matches first', () => {
    expect(searchNodeDocumentation(definitions, 'condition', 'en-US')).toEqual([
      expect.objectContaining({ nodeType: 'data.filter_rows' }),
    ]);
    expect(searchNodeDocumentation(definitions, 'regression', 'en-US')[0]?.nodeType).toBe(
      'statistics.linear_regression',
    );
  });
});
