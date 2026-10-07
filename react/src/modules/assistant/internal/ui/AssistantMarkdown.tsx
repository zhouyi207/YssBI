import { useCallback, useState, type ComponentProps } from "react";
import { defaultUrlTransform, type ExtraProps } from "react-markdown";
import {
  MarkdownTextPrimitive,
  normalizeMathDelimiters,
  type CodeHeaderProps,
} from "@assistant-ui/react-markdown";
import { useTranslation } from "react-i18next";
import { VscCheck, VscCopy } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { openExternalUrlWithDialog } from "@/features/application/window/openExternalUrlWithDialog";
import { markdownProseClass } from "@/shared/ui/markdownProseClass";
import { MarkdownLink, MarkdownLinkContext } from "@/shared/ui/MarkdownLink";
import {
  assistantLinkResource,
  openAssistantResource,
} from "@/features/application/assistant/assistantResourceActions";
import {
  markdownComponents,
  markdownRemarkPlugins,
  useMarkdownRehypePlugins,
} from "@/shared/ui/markdownRendering";
import "./assistant.css";

function CodeHeader({ language, code }: CodeHeaderProps) {
  const { t } = useTranslation();
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle");
  const label = t(
    copyState === "copied"
      ? "panel.assistantCopied"
      : copyState === "failed"
        ? "panel.assistantCopyFailed"
        : "panel.assistantCopyCode",
  );
  return (
    <div className="assistant-code-header not-prose">
      <span>{language || t("panel.assistantCode")}</span>
      <Button
        type="button"
        variant="ghost"
        size="xs"
        aria-label={label}
        onClick={async () => {
          try {
            await navigator.clipboard.writeText(code);
            setCopyState("copied");
          } catch {
            setCopyState("failed");
          }
        }}
        onBlur={() => setCopyState("idle")}
      >
        {copyState === "copied" ? <VscCheck aria-hidden /> : <VscCopy aria-hidden />}
        <span aria-live="polite">{label}</span>
      </Button>
    </div>
  );
}

function AssistantMarkdownLink(props: ComponentProps<"a"> & ExtraProps) {
  const { t } = useTranslation();
  const [failed, setFailed] = useState(false);
  const resource = props.href ? assistantLinkResource(props.href) : null;
  if (!resource) return <MarkdownLink {...props} />;
  return (
    <>
      <button
        type="button"
        className="inline cursor-pointer text-primary underline underline-offset-2"
        onClick={() => {
          setFailed(false);
          void openAssistantResource(resource).catch(() => setFailed(true));
        }}
      >
        {props.children}
      </button>
      {failed && (
        <span role="alert" className="ml-1 text-xs text-destructive">
          {t("panel.assistantResourceOpenFailed")}
        </span>
      )}
    </>
  );
}

const components = {
  ...markdownComponents,
  CodeHeader,
  a: AssistantMarkdownLink,
};

export function AssistantMarkdown() {
  const { t } = useTranslation();
  const openLink = useCallback((url: string) => openExternalUrlWithDialog(url, t), [t]);
  const rehypePlugins = useMarkdownRehypePlugins();

  return (
    <MarkdownLinkContext value={openLink}>
      <MarkdownTextPrimitive
        className={`${markdownProseClass} assistant-markdown w-full max-w-full prose-sm`}
        components={components}
        remarkPlugins={markdownRemarkPlugins}
        rehypePlugins={rehypePlugins}
        preprocess={normalizeMathDelimiters}
        urlTransform={(url) => (url.startsWith("yssbi://") ? url : defaultUrlTransform(url))}
        smooth
        defer
      />
    </MarkdownLinkContext>
  );
}
