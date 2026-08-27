import {
  AuiIf,
  ComposerPrimitive,
  ThreadPrimitive,
} from '@assistant-ui/react';
import { useTranslation } from 'react-i18next';
import { VscSend, VscSparkle } from 'react-icons/vsc';

import { Button } from '@/components/ui/button';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Separator } from '@/components/ui/separator';

export function AssistantThread() {
  const { t } = useTranslation();

  return (
    <ThreadPrimitive.Root className="flex h-full min-h-0 flex-col bg-(--workbench-bg)">
      <ThreadPrimitive.ViewportProvider>
        <ScrollArea className="min-h-0 flex-1" orientation="vertical">
          <div className="flex min-h-full min-w-0 flex-col p-3">
            <AuiIf condition={(state) => state.thread.isEmpty}>
              <Empty className="min-h-56 flex-1 px-4 py-8">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <VscSparkle aria-hidden />
                  </EmptyMedia>
                  <EmptyTitle>{t('panel.assistantEmptyTitle')}</EmptyTitle>
                  <EmptyDescription>
                    {t('panel.assistantEmptyDescription')}
                  </EmptyDescription>
                </EmptyHeader>
              </Empty>
            </AuiIf>
          </div>
        </ScrollArea>

        <Separator />
        <div className="shrink-0 p-2">
          <ComposerPrimitive.Root className="rounded-md border border-border bg-background/80 shadow-xs focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/20">
            <ComposerPrimitive.Input
              aria-label={t('panel.assistantComposerLabel')}
              className="max-h-32 min-h-16 w-full resize-none bg-transparent px-2.5 py-2 text-xs/relaxed outline-none placeholder:text-muted-foreground"
              placeholder={t('panel.assistantComposerPlaceholder')}
              submitMode="none"
            />
            <Separator />
            <div className="flex min-w-0 items-center gap-2 px-2 py-1.5">
              <span className="min-w-0 flex-1 text-[0.6875rem] leading-4 text-muted-foreground">
                {t('panel.assistantSendUnavailable')}
              </span>
              <ComposerPrimitive.Send asChild>
                <Button
                  type="submit"
                  size="icon-sm"
                  aria-label={t('panel.assistantSend')}
                  title={t('panel.assistantSend')}
                >
                  <VscSend data-icon="inline-start" aria-hidden />
                </Button>
              </ComposerPrimitive.Send>
            </div>
          </ComposerPrimitive.Root>
        </div>
      </ThreadPrimitive.ViewportProvider>
    </ThreadPrimitive.Root>
  );
}
