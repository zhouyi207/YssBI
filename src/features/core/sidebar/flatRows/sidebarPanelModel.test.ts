import { describe, expect, it } from 'vitest';
import {
  buildChartsSidebarModel,
  buildDataSidebarModel,
  buildGraphsSidebarModel,
  buildNodesSidebarModel,
  buildVariablesSidebarModel,
} from './index';

describe('structured Sidebar models', () => {
  it('keeps empty graph sections as model metadata instead of rows', () => {
    const model = buildGraphsSidebarModel({
      events: {},
      functions: {},
      expandedSections: { graphsEvent: true, graphsFunction: false },
      labels: {
        event: 'Event',
        function: 'Function',
        noEvents: 'No events',
        noFunctions: 'No functions',
      },
    });

    expect(model.sections).toEqual([
      {
        key: 'graphsEvent',
        label: 'Event',
        expanded: true,
        rows: [],
        emptyMessage: 'No events',
      },
      {
        key: 'graphsFunction',
        label: 'Function',
        expanded: false,
        rows: [],
        emptyMessage: 'No functions',
      },
    ]);
  });

  it('stores graph resources only as item rows', () => {
    const model = buildGraphsSidebarModel({
      events: { 'events/Main.yssbi-event': { name: 'Main' } },
      functions: {},
      expandedSections: {},
      labels: {
        event: 'Event',
        function: 'Function',
        noEvents: 'No events',
        noFunctions: 'No functions',
      },
    });

    expect(model.sections[0].rows).toEqual([
      {
        kind: 'graph',
        rowKey: 'graph:event:events/Main.yssbi-event',
        level: 1,
        id: 'events/Main.yssbi-event',
        name: 'Main',
        graphType: 'event',
      },
    ]);
    expect(model.sections[0].rows.map((row) => row.kind)).toEqual(['graph']);
  });

  it('represents an unmatched node search as a tab-level empty state', () => {
    const model = buildNodesSidebarModel({
      items: [],
      filterQuery: 'missing',
      expandedGroups: {},
      noMatchesMessage: 'No matching nodes',
    });

    expect(model).toEqual({
      rows: [],
      emptyState: { title: 'No matching nodes' },
    });
  });

  it('builds empty data, chart, and variable sections without empty item rows', () => {
    expect(
      buildDataSidebarModel({
        dataframes: {},
        expandedSections: {},
        labels: { data: 'Data', noData: 'No data' },
      }).sections[0].rows,
    ).toEqual([]);

    expect(
      buildChartsSidebarModel({
        worksheets: [],
        expandedSections: {},
        labels: { worksheets: 'Worksheets', noWorksheets: 'No worksheets' },
      }).sections[0].rows,
    ).toEqual([]);

    const variables = buildVariablesSidebarModel({
      localVariables: {},
      globalVariables: {},
      hasActiveGraph: false,
      expandedSections: {},
      labels: {
        local: 'Local',
        global: 'Global',
        noLocal: 'No local variables',
        noGlobal: 'No global variables',
        noActiveGraph: 'No active graph',
      },
    });
    expect(variables.sections.map((section) => section.emptyMessage)).toEqual([
      'No active graph',
      'No global variables',
    ]);
  });
});
