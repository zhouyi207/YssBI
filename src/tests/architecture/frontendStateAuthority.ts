export type FrontendStateAuthorityClass =
  | 'backend-base'
  | 'optimistic-overlay'
  | 'local-draft'
  | 'frontend-ui';

export type FrontendWriterLayer = 'Application' | 'CoreUi';
export type FrontendStateMemberKind = 'field' | 'action';
export type FrontendStateReaderLayer = 'Views' | 'Application' | 'Core';

export interface FrontendStateAuthorityEntry {
  readonly storeModule: string;
  readonly member: string;
  readonly memberKind: FrontendStateMemberKind;
  readonly authority: FrontendStateAuthorityClass;
  readonly writes: readonly string[];
  readonly writerModule: string | null;
  readonly writerLayer: FrontendWriterLayer | null;
  readonly readerLayers: readonly FrontendStateReaderLayer[];
}

export interface FrontendStateAuthorityMember extends FrontendStateAuthorityEntry {
  readonly sourceLayer?: FrontendStateReaderLayer;
  readonly delegatesTo?: string | null;
}

export interface FrontendStateAuthorityFinding {
  readonly ruleId:
    | 'frontend-state-authority-missing-member'
    | 'frontend-state-authority-writer'
    | 'frontend-state-authority-action-writer'
    | 'frontend-state-authority-action-cycle'
    | 'frontend-state-authority-unresolved-delegate';
  readonly storeModule: string;
  readonly member: string;
  readonly memberKind: FrontendStateMemberKind;
  readonly canonicalWritePath: string | null;
}

const memberKey = (member: Pick<FrontendStateAuthorityEntry, 'storeModule' | 'member' | 'memberKind'>): string => (
  `${member.storeModule}::${member.memberKind}::${member.member}`
);

const canonicalPath = (path: string): string => path
  .replace(/\[[^\]]+\]/g, '.*')
  .replace(/\b(?:[A-Za-z_$][\w$]*|\d+)\b(?=\.dirty$)/, '*');

export const FRONTEND_STATE_AUTHORITY: readonly FrontendStateAuthorityEntry[] = [
  {
    storeModule: 'src/features/core/dataStore/projectIOStore.ts',
    member: 'projectInstanceId',
    memberKind: 'field',
    authority: 'backend-base',
    writes: ['projectInstanceId'],
    writerModule: '@/features/core/project/publication',
    writerLayer: 'Application',
    readerLayers: ['Views', 'Application'],
  },
  {
    storeModule: 'src/features/core/resource/documentStateStore.ts',
    member: 'documents.*.dirty',
    memberKind: 'field',
    authority: 'local-draft',
    writes: ['documents.*.dirty'],
    writerModule: '@/features/core/resource/ui',
    writerLayer: 'CoreUi',
    readerLayers: ['Views', 'Application', 'Core'],
  },
  {
    storeModule: 'src/features/core/worksheet/worksheetStore.ts',
    member: 'updateDocument',
    memberKind: 'action',
    authority: 'local-draft',
    writes: ['draftsByPath.*', 'dirtyByPath.*'],
    writerModule: '@/features/core/worksheet/ui',
    writerLayer: 'CoreUi',
    readerLayers: ['Views', 'Application'],
  },
];

export function auditFrontendStateAuthority(
  members: readonly FrontendStateAuthorityMember[],
  manifest: readonly FrontendStateAuthorityEntry[] = FRONTEND_STATE_AUTHORITY,
): readonly FrontendStateAuthorityFinding[] {
  const entries = new Map(manifest.map((entry) => [memberKey(entry), entry]));
  const findings: FrontendStateAuthorityFinding[] = [];
  const actionMembers = new Map(
    members
      .filter((member) => member.memberKind === 'action')
      .map((member) => [memberKey(member), member]),
  );

  for (const member of members) {
    const entry = entries.get(memberKey(member));
    if (!entry) {
      findings.push({
        ruleId: 'frontend-state-authority-missing-member',
        storeModule: member.storeModule,
        member: member.member,
        memberKind: member.memberKind,
        canonicalWritePath: member.writes[0] ? canonicalPath(member.writes[0]) : null,
      });
      continue;
    }

    if (member.sourceLayer === 'Views' && entry.writerLayer !== 'CoreUi') {
      findings.push({
        ruleId: 'frontend-state-authority-writer',
        storeModule: member.storeModule,
        member: member.member,
        memberKind: member.memberKind,
        canonicalWritePath: entry.writes[0] ? canonicalPath(entry.writes[0]) : null,
      });
    }
    if (member.memberKind === 'action' && member.writerLayer !== entry.writerLayer) {
      findings.push({
        ruleId: 'frontend-state-authority-action-writer',
        storeModule: member.storeModule,
        member: member.member,
        memberKind: member.memberKind,
        canonicalWritePath: entry.writes[0] ? canonicalPath(entry.writes[0]) : null,
      });
    }
  }

  for (const member of actionMembers.values()) {
    const delegate = member.delegatesTo;
    if (!delegate) continue;
    if (!actionMembers.has(delegate)) {
      findings.push({
        ruleId: 'frontend-state-authority-unresolved-delegate',
        storeModule: member.storeModule,
        member: member.member,
        memberKind: member.memberKind,
        canonicalWritePath: member.writes[0] ? canonicalPath(member.writes[0]) : null,
      });
      continue;
    }
    const visited = new Set<string>();
    let current: string | undefined = memberKey(member);
    while (current) {
      if (!visited.add(current)) {
        findings.push({
          ruleId: 'frontend-state-authority-action-cycle',
          storeModule: member.storeModule,
          member: member.member,
          memberKind: member.memberKind,
          canonicalWritePath: member.writes[0] ? canonicalPath(member.writes[0]) : null,
        });
        break;
      }
      const next: string | null | undefined = actionMembers.get(current)?.delegatesTo;
      current = next ?? undefined;
    }
  }

  return findings;
}
