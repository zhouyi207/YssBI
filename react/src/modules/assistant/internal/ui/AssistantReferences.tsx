import { useMemo, useState } from "react";
import { useAuiState } from "@assistant-ui/react";
import { useTranslation } from "react-i18next";
import { VscMention, VscClose, VscCheck, VscFile } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { resourceKey } from "@/features/core/resource";
import { useResourceRead } from "@/features/core/resource/read";
import {
  useAssistantHarnessActions,
  useAssistantHarnessSnapshot,
} from "@/features/application/assistant/AssistantRuntimeProvider";
import { openAssistantResource } from "@/features/application/assistant/assistantResourceActions";
import type { ResourceRef } from "@/shared/types/domain/resource";
import type { HarnessResourceReference } from "@/services/assistant/harnessContract";

export function AssistantResourcePicker({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const resources = useResourceRead((state) => state.resources);
  const selected = useAssistantHarnessSnapshot((state) => state.draftResources);
  const sessionId = useAssistantHarnessSnapshot((state) => state.sessionId);
  const { addResource, removeResource } = useAssistantHarnessActions();
  const matches = useMemo(() => {
    const text = query.trim().toLocaleLowerCase();
    return Object.values(resources)
      .filter(
        (resource) =>
          resource.exists && `${resource.name} ${resource.id}`.toLocaleLowerCase().includes(text),
      )
      .sort((left, right) => left.name.localeCompare(right.name));
  }, [resources, query]);
  const keys = new Set(selected.map(resourceKey));
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          disabled={!sessionId}
          title={t("panel.assistantAttachResource")}
          aria-label={t("panel.assistantAttachResource")}
        >
          <VscMention aria-hidden />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" side="top" className="gap-2">
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={t("panel.assistantFindResource")}
          aria-label={t("panel.assistantFindResource")}
        />
        <div
          className="max-h-64 space-y-1 overflow-y-auto"
          role="group"
          aria-label={t("panel.assistantAttachResource")}
        >
          {matches.map((resource) => {
            const ref: ResourceRef = { kind: resource.kind, id: resource.id };
            const chosen = keys.has(resourceKey(ref));
            return (
              <Button
                key={resourceKey(ref)}
                type="button"
                variant="ghost"
                className="h-auto w-full justify-start gap-2 px-2 py-1.5 text-left"
                aria-pressed={chosen}
                onClick={() => (chosen ? removeResource(ref) : addResource(ref))}
              >
                {chosen ? (
                  <VscCheck aria-hidden className="shrink-0" />
                ) : (
                  <VscFile aria-hidden className="shrink-0 text-muted-foreground" />
                )}
                <span className="min-w-0">
                  <span className="block truncate text-xs">{resource.name}</span>
                  <span
                    className="block truncate text-[10px] font-normal text-muted-foreground"
                    title={resource.id}
                  >
                    {resource.id}
                  </span>
                </span>
              </Button>
            );
          })}
          {matches.length === 0 && (
            <p role="status" className="px-2 py-3 text-muted-foreground">
              {t("panel.assistantNoResources")}
            </p>
          )}
        </div>
        <p className="text-[10px] leading-4 text-muted-foreground">
          {t("panel.assistantReferenceHint")}
        </p>
      </PopoverContent>
    </Popover>
  );
}

function ReferenceChip({
  resource,
  name,
  onRemove,
}: {
  resource: ResourceRef;
  name?: string;
  onRemove?: (resource: ResourceRef) => void;
}) {
  const { t } = useTranslation();
  const meta = useResourceRead((state) => state.resources[resourceKey(resource)]);
  const [failed, setFailed] = useState(false);
  const [opening, setOpening] = useState(false);
  return (
    <span className="inline-flex max-w-full items-center rounded-md border border-border/70 bg-background/60 text-[11px]">
      <button
        type="button"
        disabled={!meta?.exists || opening}
        className="min-w-0 truncate px-2 py-0.5 text-left disabled:opacity-50"
        title={`${resource.id}${!meta?.exists || failed ? ` · ${t("panel.assistantResourceUnavailable")}` : ""}`}
        onClick={async () => {
          setOpening(true);
          setFailed(false);
          try {
            await openAssistantResource(resource);
          } catch {
            setFailed(true);
          } finally {
            setOpening(false);
          }
        }}
      >
        @{name ?? meta?.name ?? resource.id}
      </button>
      {onRemove && (
        <button
          type="button"
          className="shrink-0 px-1 py-0.5 hover:text-destructive"
          aria-label={t("panel.assistantRemoveReference", {
            name: name ?? meta?.name ?? resource.id,
          })}
          onClick={() => onRemove(resource)}
        >
          <VscClose aria-hidden />
        </button>
      )}
      {failed && (
        <span role="alert" className="px-1 text-destructive">
          {t("panel.assistantResourceOpenFailed")}
        </span>
      )}
    </span>
  );
}

export function AssistantReferenceChips({
  resources,
  onRemove,
}: {
  resources: readonly (ResourceRef & { name?: string })[];
  onRemove?: (resource: ResourceRef) => void;
}) {
  if (resources.length === 0) return null;
  return (
    <div className="flex flex-wrap gap-1 py-1">
      {resources.map((resource) => (
        <ReferenceChip
          key={resourceKey(resource)}
          resource={resource}
          name={resource.name}
          onRemove={onRemove}
        />
      ))}
    </div>
  );
}

export function AssistantDraftReferences() {
  const resources = useAssistantHarnessSnapshot((state) => state.draftResources);
  const { removeResource } = useAssistantHarnessActions();
  return resources.length > 0 ? (
    <div className="px-3 pt-1">
      <AssistantReferenceChips resources={resources} onRemove={removeResource} />
    </div>
  ) : null;
}

export function AssistantUserReferences() {
  // This metadata originates from the strictly parsed TurnStarted payload.
  const resources = useAuiState((state) => state.message.metadata.custom.resources) as
    | readonly HarnessResourceReference[]
    | undefined;
  return (
    <AssistantReferenceChips
      resources={resources?.map((entry) => ({ ...entry.resource, name: entry.name })) ?? []}
    />
  );
}
