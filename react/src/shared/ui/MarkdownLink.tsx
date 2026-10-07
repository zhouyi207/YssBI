import { createContext, useContext, useState, type ComponentProps, type MouseEvent } from "react";
import type { ExtraProps } from "react-markdown";
import { useTranslation } from "react-i18next";
import { openExternalUrl } from "@/shared/utils/openExternalUrl";
import { parseReferenceUrl } from "@/shared/utils/referenceUrl";

type OpenMarkdownLink = (url: string, title: string) => Promise<void>;

export const MarkdownLinkContext = createContext<OpenMarkdownLink>(openExternalUrl);

export function MarkdownLink({
  node: _node,
  href,
  children,
  title,
  ...props
}: ComponentProps<"a"> & ExtraProps) {
  const open = useContext(MarkdownLinkContext);
  const { t } = useTranslation();
  const [failed, setFailed] = useState(false);
  if (href?.startsWith("#")) {
    return (
      <a
        {...props}
        href={href}
        title={title}
        onClick={(event) => {
          event.preventDefault();
          let id = href.slice(1);
          try {
            id = decodeURIComponent(id);
          } catch {
            /* Keep malformed fragments literal. */
          }
          if (!id) return;
          event.currentTarget
            .closest(".prose-app")
            ?.querySelector(`#${CSS.escape(id)}`)
            ?.scrollIntoView({ block: "nearest" });
        }}
        onAuxClick={(event) => event.preventDefault()}
      >
        {children}
      </a>
    );
  }
  const url = parseReferenceUrl(href);
  if (!url) return <span>{children}</span>;

  const activate = (event: MouseEvent<HTMLAnchorElement>) => {
    if (event.type === "auxclick" && event.button !== 1) return;
    event.preventDefault();
    setFailed(false);
    void open(url.href, event.currentTarget.textContent?.trim() || title || url.hostname).catch(
      () => setFailed(true),
    );
  };
  return (
    <>
      <a
        {...props}
        href={url.href}
        title={title}
        target="_blank"
        rel="noopener noreferrer"
        onClick={activate}
        onAuxClick={activate}
      >
        {children}
      </a>
      {failed && (
        <span role="alert" className="ml-1 text-destructive">
          {t("notifications.externalUrl.openFailed")}
        </span>
      )}
    </>
  );
}
