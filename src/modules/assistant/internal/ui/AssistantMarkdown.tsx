import { useState, type ReactNode } from "react";
import {
  MarkdownTextPrimitive,
  normalizeMathDelimiters,
  type CodeHeaderProps,
} from "@assistant-ui/react-markdown";
import { useTranslation } from "react-i18next";
import { VscCheck, VscCopy } from "react-icons/vsc";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";
import rehypeKatex from "rehype-katex";
import { Button } from "@/components/ui/button";
import { openExternalUrlWithDialog } from "@/features/application/window/openExternalUrlWithDialog";
import "katex/dist/katex.min.css";
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
    <div className="assistant-code-header">
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

function MarkdownLink({ children, href }: { children?: ReactNode; href?: string }) {
  const { t } = useTranslation();
  if (!href || !/^https?:\/\//i.test(href)) return <span>{children}</span>;
  return (
    <a
      href={href}
      onClick={(event) => {
        event.preventDefault();
        void openExternalUrlWithDialog(href, t);
      }}
    >
      {children}
    </a>
  );
}

const components = {
  CodeHeader,
  a: MarkdownLink,
};
const remarkPlugins = [remarkGfm, remarkMath];
const rehypePlugins = [rehypeKatex];

export function AssistantMarkdown() {
  return (
    <MarkdownTextPrimitive
      className="assistant-markdown"
      components={components}
      remarkPlugins={remarkPlugins}
      rehypePlugins={rehypePlugins}
      preprocess={normalizeMathDelimiters}
      smooth
      defer
    />
  );
}
