import {
  DIAGNOSTIC_DOMAINS,
  type DiagnosticDomain,
} from '@/shared/types/domain/diagnostics';

export type LogDomainId = 'all' | DiagnosticDomain;

export const LOG_DOMAIN_ORDER: readonly LogDomainId[] = [
  'all',
  ...DIAGNOSTIC_DOMAINS,
];

const LOG_DOMAIN_IDS = new Set<LogDomainId>(LOG_DOMAIN_ORDER);
const LOG_DOMAIN_TITLES: Readonly<Record<LogDomainId, string>> = {
  all: 'All',
  application: 'Application',
  execution: 'Execution',
  system: 'System',
  graph: 'Graph',
  data: 'Data',
  ui: 'UI',
};

export function isLogDomainId(value: unknown): value is LogDomainId {
  return typeof value === 'string' && LOG_DOMAIN_IDS.has(value as LogDomainId);
}

export function logDomainTitle(domain: LogDomainId): string {
  return LOG_DOMAIN_TITLES[domain];
}

export function logDomainPanelId(domain: LogDomainId): string {
  return `logs-domain:${domain}`;
}
