import { describe, expect, it } from 'vitest';
import { buildGraphResourceMeta } from '@/features/core/resource';
import type { GraphEntityBucket } from '@/features/core/dataStore/graphEntityAccess';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import {
  collectCallFunctionIssuesForBucket,
  getCallFunctionIssueForNode,
  getFunctionResourceName,
  isFunctionResourceAvailable,
} from './callFunctionDiagnostics';

describe('callFunctionDiagnostics', () => {
  const resources = {
    [buildGraphResourceMeta('function', 'functions/A.yssbi-function', 'A').uri]: buildGraphResourceMeta(
      'function',
      'functions/A.yssbi-function',
      'A',
    ),
    [buildGraphResourceMeta('function', 'functions/Gone.yssbi-function', 'Gone', { exists: false }).uri]:
      buildGraphResourceMeta('function', 'functions/Gone.yssbi-function', 'Gone', { exists: false }),
  };

  it('detects empty and missing Call Function targets', () => {
    const graphPath = 'events/Main.yssbi-event';
    const call = makeEditorProjectionFixture({
      graphPath,
      nodeId: 'call-1',
      nodeTypeId: 'yssbi.project.function.call',
      title: 'Localized call title',
    });
    const stableNodeType = call.projection.nodes[0].nodeTypeId;
    const stableTitle = call.projection.nodes[0].display.title;
    const bucket = {
      basis: {
        graphPath,
        graphRevision: 1,
        registryFingerprint: [],
        resourceVersions: {},
      },
      sourceRevision: 1,
      requestGeneration: 1,
      diagnostics: [],
      hasBlockingDiagnostics: false,
      nodes: {
        'call-empty': {
          id: 'call-empty',
          graphPath,
          nodeType: stableNodeType,
          title: stableTitle,
          category: [],
          position: { x: 0, y: 0 },
          inputs: [],
          outputs: [],
        },
        'call-missing': {
          id: 'call-missing',
          graphPath,
          nodeType: stableNodeType,
          subGraphPath: 'functions/Gone.yssbi-function',
          title: stableTitle,
          category: [],
          position: { x: 0, y: 0 },
          inputs: [],
          outputs: [],
        },
        'legacy-call': {
          id: 'legacy-call',
          graphPath,
          nodeType: 'Functions:Call Function',
          subGraphPath: 'functions/Gone.yssbi-function',
          title: 'Legacy call label',
          category: [],
          position: { x: 0, y: 0 },
          inputs: [],
          outputs: [],
        },
        'call-ok': {
          id: 'call-ok',
          graphPath,
          nodeType: stableNodeType,
          subGraphPath: 'functions/A.yssbi-function',
          title: stableTitle,
          category: [],
          position: { x: 0, y: 0 },
          inputs: [],
          outputs: [],
        },
      },
      pins: {},
      connections: {},
      graphNodes: ['call-empty', 'call-missing', 'legacy-call', 'call-ok'],
      nodePins: {},
      pinConnections: {},
    } satisfies GraphEntityBucket;

    expect(getCallFunctionIssueForNode('events/Main.yssbi-event', bucket.nodes['call-empty'], resources)?.kind).toBe(
      'empty_target',
    );
    expect(getCallFunctionIssueForNode('events/Main.yssbi-event', bucket.nodes['call-missing'], resources)?.kind).toBe(
      'missing_target',
    );
    expect(getCallFunctionIssueForNode('events/Main.yssbi-event', bucket.nodes['call-ok'], resources)).toBeNull();
    expect(getCallFunctionIssueForNode('events/Main.yssbi-event', bucket.nodes['legacy-call'], resources)).toBeNull();

    expect(collectCallFunctionIssuesForBucket('events/Main.yssbi-event', bucket, resources)).toHaveLength(2);
  });

  it('checks function resource existence via ResourceStore meta', () => {
    expect(isFunctionResourceAvailable(resources, 'functions/A.yssbi-function')).toBe(true);
    expect(isFunctionResourceAvailable(resources, 'functions/Gone.yssbi-function')).toBe(false);
    expect(isFunctionResourceAvailable(resources, 'functions/Missing.yssbi-function')).toBe(false);
    expect(getFunctionResourceName(resources, 'functions/A.yssbi-function')).toBe('A');
    expect(getFunctionResourceName(resources, 'functions/Gone.yssbi-function')).toBeUndefined();
  });
});
