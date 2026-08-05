import { lookupGraphResource } from '@/features/core/resource/resourceSelectors';
import type { ProjectResourceMeta, ResourceKey } from '@/features/core/resource/resourceTypes';
import { isCallFunctionNodeType } from '@/features/domain/nodeCatalog';
import type { GraphEntityBucket } from '@/features/core/dataStore/graphEntityAccess';
import type { NodeData } from '@/shared/types/store/graph';

export type CallFunctionIssueKind = 'empty_target' | 'missing_target';

export interface CallFunctionIssue {
  graphPath: string;
  nodeId: string;
  kind: CallFunctionIssueKind;
  subGraphPath?: string;
}

export function isFunctionResourceAvailable(
  resources: Record<ResourceKey, ProjectResourceMeta>,
  functionPath: string,
): boolean {
  const path = functionPath.trim();
  if (!path) return false;
  return lookupGraphResource(resources, path, 'function')?.exists === true;
}

export function getFunctionResourceName(
  resources: Record<ResourceKey, ProjectResourceMeta>,
  functionPath: string,
): string | undefined {
  const path = functionPath.trim();
  if (!path) return undefined;
  const meta = lookupGraphResource(resources, path, 'function');
  return meta?.exists ? meta.name : undefined;
}

export function getCallFunctionIssueForNode(
  graphPath: string,
  node: Pick<NodeData, 'id' | 'nodeType' | 'subGraphPath'>,
  resources: Record<ResourceKey, ProjectResourceMeta>,
): CallFunctionIssue | null {
  if (!isCallFunctionNodeType(node.nodeType)) return null;

  const subGraphPath = node.subGraphPath?.trim();
  if (!subGraphPath) {
    return { graphPath, nodeId: node.id, kind: 'empty_target' };
  }

  if (!isFunctionResourceAvailable(resources, subGraphPath)) {
    return { graphPath, nodeId: node.id, kind: 'missing_target', subGraphPath };
  }

  return null;
}

export function collectCallFunctionIssuesForBucket(
  graphPath: string,
  bucket: GraphEntityBucket,
  resources: Record<ResourceKey, ProjectResourceMeta>,
): CallFunctionIssue[] {
  const issues: CallFunctionIssue[] = [];
  for (const node of Object.values(bucket.nodes) as NodeData[]) {
    const issue = getCallFunctionIssueForNode(graphPath, node, resources);
    if (issue) issues.push(issue);
  }
  return issues;
}

export function countCallFunctionIssuesByGraph(
  graphEntities: Record<string, GraphEntityBucket>,
  resources: Record<ResourceKey, ProjectResourceMeta>,
): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const [graphPath, bucket] of Object.entries(graphEntities)) {
    const issueCount = collectCallFunctionIssuesForBucket(graphPath, bucket, resources).length;
    if (issueCount > 0) counts[graphPath] = issueCount;
  }
  return counts;
}
