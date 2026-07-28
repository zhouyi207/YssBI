import { describe, expect, it } from 'vitest';
import { buildGraphResourceMeta } from '@/features/core/resource';
import type { GraphEntityBucket } from '@/features/core/dataStore/graphEntityAccess';
import { CALL_FUNCTION_NODE_TYPE } from '@/features/domain/nodeDefinition';
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
          nodeType: CALL_FUNCTION_NODE_TYPE,
          title: 'Call',
          category: [],
          position: { x: 0, y: 0 },
          inputs: [],
          outputs: [],
        },
        'call-missing': {
          id: 'call-missing',
          graphPath,
          nodeType: CALL_FUNCTION_NODE_TYPE,
          subGraphPath: 'functions/Gone.yssbi-function',
          title: 'Call',
          category: [],
          position: { x: 0, y: 0 },
          inputs: [],
          outputs: [],
        },
        'call-ok': {
          id: 'call-ok',
          graphPath,
          nodeType: CALL_FUNCTION_NODE_TYPE,
          subGraphPath: 'functions/A.yssbi-function',
          title: 'Call',
          category: [],
          position: { x: 0, y: 0 },
          inputs: [],
          outputs: [],
        },
      },
      pins: {},
      connections: {},
      graphNodes: ['call-empty', 'call-missing', 'call-ok'],
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
