import { useTranslation } from 'react-i18next';
import { Card, CardContent, CardHeader } from '@/components/ui/card';
import type { FunctionCallSiteDTO } from '@/shared/types/dto';
import { openGraphResource, resolveGraphResourceMeta } from '@/features/application/editor/openGraphResource';
import { cn } from '@/lib/utils';
import { DetailBadge, DetailSectionHeader, DetailText } from './DetailText';
import { detailEmptyHintClass, detailListItemClass } from './detailStyles';

interface FunctionCallSitesSectionProps {
  callSites: FunctionCallSiteDTO[];
  loading?: boolean;
}

export function FunctionCallSitesSection({
  callSites,
  loading = false,
}: FunctionCallSitesSectionProps) {
  const { t } = useTranslation();

  return (
    <Card className="rounded-lg bg-card/80 py-0 shadow-xs">
      <CardHeader className="px-3 py-2">
        <DetailSectionHeader level="subsection">
          {t('detail.callSites.title', { count: callSites.length })}
        </DetailSectionHeader>
      </CardHeader>
      <CardContent className="px-3 pb-3 pt-0">
        {loading ? (
          <DetailText tone="muted">{t('detail.callSites.loading')}</DetailText>
        ) : callSites.length === 0 ? (
          <div className={detailEmptyHintClass}>{t('detail.callSites.empty')}</div>
        ) : (
          <ul className="space-y-1.5">
            {callSites.map((site) => {
              const meta = resolveGraphResourceMeta(site.callerGraphPath);
              const name = meta?.name ?? site.callerGraphPath;
              const callCount = site.nodeIds.length;

              return (
                <li key={site.callerGraphPath}>
                  <button
                    type="button"
                    className={cn(
                      detailListItemClass,
                      'w-full gap-3 text-left hover:text-[var(--accent-color)]',
                    )}
                    onClick={() => {
                      void openGraphResource(site.callerGraphPath, meta?.type);
                    }}
                  >
                    <span className="min-w-0 flex-1 truncate font-medium">{name}</span>
                    <DetailBadge title={t('detail.callSites.references', { count: callCount })}>
                      {t('detail.callSites.references', { count: callCount })}
                    </DetailBadge>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
