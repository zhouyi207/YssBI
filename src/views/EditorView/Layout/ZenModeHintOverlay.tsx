import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { editorDropPreviewLabelClass } from "./editorDropPreviewStyles";

const AUTO_DISMISS_MS = 4000;

export function ZenModeHintOverlay() {
  const { t } = useTranslation();
  const zenMode = useLayoutStore((s) => s.zenMode);
  const [visible, setVisible] = useState(false);
  const prevZenRef = useRef(false);

  useEffect(() => {
    const entered = zenMode && !prevZenRef.current;
    prevZenRef.current = zenMode;

    if (!entered) {
      if (!zenMode) setVisible(false);
      return;
    }

    setVisible(true);
    const timer = window.setTimeout(() => setVisible(false), AUTO_DISMISS_MS);
    return () => window.clearTimeout(timer);
  }, [zenMode]);

  useEffect(() => {
    if (!visible) return;

    const dismiss = () => setVisible(false);
    window.addEventListener("keydown", dismiss, { once: true });
    window.addEventListener("pointerdown", dismiss, { once: true });
    return () => {
      window.removeEventListener("keydown", dismiss);
      window.removeEventListener("pointerdown", dismiss);
    };
  }, [visible]);

  if (!zenMode || !visible) return null;

  return (
    <div
      className="pointer-events-none fixed bottom-8 left-1/2 z-[150] -translate-x-1/2"
      role="status"
      aria-live="polite"
    >
      <div className={editorDropPreviewLabelClass}>
        {t("workbench.exitZenModeHint")}{" "}
        <kbd className="ml-1 rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[10px] font-semibold">
          Esc
        </kbd>
      </div>
    </div>
  );
}
