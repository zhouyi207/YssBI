import { useEffect, useRef, useState } from "react";
import { TextMessagePartProvider, type SourceMessagePartComponent } from "@assistant-ui/react";
import { useTranslation } from "react-i18next";
import { VscFile, VscReferences } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { resourceKey } from "@/features/core/resource";
import { useResourceRead } from "@/features/core/resource/read";
import { openAssistantResource } from "@/features/application/assistant/assistantResourceActions";
import { useAssistantHarnessSnapshot } from "@/features/application/assistant/AssistantRuntimeProvider";
import { HarnessService, type HarnessKnowledgeCitation } from "@/services/assistant/harnessService";
import type { HarnessArtifact, HarnessResultReference } from "@/services/assistant/harnessContract";
import { openInspectableResult } from "@/features/application/execution/openInspectableResult";
import { resultRef } from "@/features/application/results";
import { AssistantMarkdown } from "./AssistantMarkdown";
import {
  captureProjectReadContext,
  useProjectIOStore,
} from "@/features/application/project/projectIOStore";
import type { CitationDetail } from "@/services/assistant/knowledgeService";

function ArtifactCard({ artifact }: { artifact: HarnessArtifact }) {
  const { t } = useTranslation();
  const meta = useResourceRead((snapshot) => snapshot.resources[resourceKey(artifact.resource)]);
  const [failed, setFailed] = useState(false);
  const [opening, setOpening] = useState(false);
  const unavailable = artifact.deleted || !meta?.exists;
  return (
    <div className="my-2 rounded-lg border border-border/60 bg-background p-2.5 text-xs">
      <Button
        type="button"
        variant="ghost"
        size="xs"
        className="h-auto w-full justify-start whitespace-normal text-left"
        disabled={unavailable || opening}
        onClick={async () => {
          setOpening(true);
          setFailed(false);
          try {
            await openAssistantResource(artifact.resource);
          } catch {
            setFailed(true);
          } finally {
            setOpening(false);
          }
        }}
      >
        <VscFile aria-hidden className="shrink-0" />
        <span className="min-w-0 flex-1 wrap-anywhere">{meta?.name ?? artifact.resource.id}</span>
        <span className="shrink-0 text-muted-foreground">
          {t(unavailable ? "panel.assistantResourceUnavailable" : "panel.assistantOpenResource")}
        </span>
      </Button>
      {meta?.revision !== undefined && meta.revision !== artifact.revision && !unavailable && (
        <p className="mt-1 text-muted-foreground">{t("panel.assistantResourceUpdated")}</p>
      )}
      {failed && (
        <p role="alert" className="mt-1 text-destructive">
          {t("panel.assistantResourceOpenFailed")}
        </p>
      )}
    </div>
  );
}

function ResultCard({ result }: { result: HarnessResultReference }) {
  const { t } = useTranslation();
  const [opening, setOpening] = useState(false);
  const [failed, setFailed] = useState(false);
  return (
    <div className="my-2 rounded-lg border border-border/60 p-2 text-xs">
      <Button
        type="button"
        variant="ghost"
        size="xs"
        disabled={opening}
        className="h-auto max-w-full whitespace-normal text-left"
        onClick={async () => {
          setOpening(true);
          setFailed(false);
          try {
            setFailed(!(await openInspectableResult(resultRef(result))));
          } catch {
            setFailed(true);
          } finally {
            setOpening(false);
          }
        }}
      >
        <VscFile aria-hidden className="shrink-0" />
        <span className="min-w-0 wrap-anywhere">
          {t("panel.assistantOpenResult")} · {result.output}
        </span>
      </Button>
      {failed && (
        <p role="alert" className="mt-1 text-destructive">
          {t("panel.assistantResultUnavailable")}
        </p>
      )}
    </div>
  );
}

export function AssistantArtifacts({
  artifacts,
  results = [],
}: {
  artifacts: readonly HarnessArtifact[];
  results?: readonly HarnessResultReference[];
}) {
  return (
    <>
      {artifacts.map((artifact, index) => (
        <ArtifactCard key={`${resourceKey(artifact.resource)}:${index}`} artifact={artifact} />
      ))}
      {results.map((result) => (
        <ResultCard key={`${result.executionSessionId}:${result.resultId}`} result={result} />
      ))}
    </>
  );
}

export const AssistantSourceCard: SourceMessagePartComponent = ({ title, providerMetadata }) => {
  const { t } = useTranslation();
  const sessionId = useAssistantHarnessSnapshot((state) => state.sessionId);
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const citation = providerMetadata?.yssbi as HarnessKnowledgeCitation | undefined;
  const [open, setOpen] = useState(false);
  const [detail, setDetail] = useState<CitationDetail | null>(null);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  const request = useRef(0);
  useEffect(() => {
    ++request.current;
    setOpen(false);
    setDetail(null);
    setLoading(false);
    setFailed(false);
    return () => {
      ++request.current;
    };
  }, [sessionId, projectInstanceId, citation?.chunkId]);

  async function inspect(openResource: boolean) {
    const context = captureProjectReadContext(projectInstanceId);
    if (!context || !sessionId || !citation) return;
    const current = ++request.current;
    setLoading(true);
    setFailed(false);
    setDetail(null);
    try {
      const result = await HarnessService.inspectCitation(sessionId, citation);
      if (current !== request.current || !context.isCurrent()) return;
      setDetail(result);
      if (openResource && result.resource) await openAssistantResource(result.resource);
    } catch {
      if (current === request.current && context.isCurrent()) setFailed(true);
    } finally {
      if (current === request.current) setLoading(false);
    }
  }
  return (
    <div className="my-2 rounded-lg border border-border/60 p-2 text-xs">
      <Button
        type="button"
        variant="ghost"
        size="xs"
        className="h-auto max-w-full whitespace-normal text-left"
        aria-expanded={open}
        disabled={!citation || loading}
        onClick={() => {
          setOpen(!open);
          if (!open) void inspect(false);
        }}
      >
        <VscReferences aria-hidden className="shrink-0" />
        {t("panel.assistantSource")}: {title}
      </Button>
      {open && (
        <div className="mt-2 space-y-2">
          {detail?.resource && (
            <Button
              type="button"
              variant="outline"
              size="xs"
              disabled={loading}
              onClick={() => void inspect(true)}
            >
              <VscFile aria-hidden />
              {t("panel.assistantOpenSource")}
            </Button>
          )}
          {loading && <p role="status">{t("panel.assistantLoadingDetails")}</p>}
          {failed && (
            <p role="alert" className="text-destructive">
              {t("panel.assistantCitationUnavailable")}
            </p>
          )}
          {detail !== null && (
            <TextMessagePartProvider text={detail.text}>
              <AssistantMarkdown />
            </TextMessagePartProvider>
          )}
        </div>
      )}
    </div>
  );
};
