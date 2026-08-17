import {
  DIAGNOSTIC_DOMAINS,
  type DiagnosticDomain,
} from '@/shared/types/dto/diagnostics';

export type LogDomainTabId = 'all' | DiagnosticDomain;

export const LOG_DOMAIN_TAB_ORDER: LogDomainTabId[] = ['all', ...DIAGNOSTIC_DOMAINS];

export function domainsForLogDomainTab(tab: LogDomainTabId): Set<DiagnosticDomain> | null {
  return tab === 'all' ? null : new Set([tab]);
}
