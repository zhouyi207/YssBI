import { describe, expect, it } from 'vitest';
import { buildDataSidebarModel } from './index';

describe('structured Sidebar models', () => {
  it('builds an empty Data section without an empty item row', () => {
    expect(
      buildDataSidebarModel({
        dataframes: {},
        expandedSections: {},
        labels: { data: 'Data', noData: 'No data' },
      }).sections[0].rows,
    ).toEqual([]);
  });
});
