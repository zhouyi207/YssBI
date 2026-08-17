import { useTranslation } from "react-i18next";
import { VscFolderOpened, VscNewFile, VscSymbolMethod } from "react-icons/vsc";
import { useEditorSessionCommandsContext } from "@/features/application/editor";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { BrandMark } from "@/shared/ui/BrandMark";

export const WatermarkView = () => {
  const { t } = useTranslation();
  const { addEvent, addFunction, importGraph } = useEditorSessionCommandsContext();

  return (
    <div className="relative flex h-full w-full select-none items-center justify-center overflow-hidden bg-[var(--workbench-bg)] px-6 py-10">
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 opacity-70"
        style={{
          backgroundImage: 'radial-gradient(circle at 1px 1px, var(--grid-lines) 1px, transparent 1.2px)',
          backgroundSize: '40px 40px',
          maskImage: 'radial-gradient(circle at center, black 0%, transparent 68%)',
        }}
      />
      <svg aria-hidden="true" viewBox="0 0 720 320" className="pointer-events-none absolute h-[78%] w-[78%] max-w-[920px] text-[var(--accent-color)] opacity-25">
        <path d="M22 76h124c52 0 58 76 110 76h208c52 0 58-76 110-76h124" fill="none" stroke="currentColor" strokeWidth="1" />
        <path d="M22 244h124c52 0 58-76 110-76h208c52 0 58 76 110 76h124" fill="none" stroke="currentColor" strokeWidth="1" />
        <path className="project-flow-signal" d="M22 76h124c52 0 58 76 110 76h208c52 0 58 92 110 92h124" fill="none" stroke="currentColor" strokeWidth="1.25" />
        <g fill="var(--workbench-bg)" stroke="currentColor" strokeWidth="1.4">
          <circle cx="22" cy="76" r="5" />
          <circle cx="22" cy="244" r="5" />
          <circle cx="360" cy="160" r="7" />
          <circle cx="698" cy="76" r="5" />
          <circle cx="698" cy="244" r="5" />
        </g>
      </svg>

      <div className="relative z-10 flex w-full max-w-[620px] flex-col items-center">
        <div className="relative mb-6">
          <div aria-hidden="true" className="absolute inset-0 scale-[2.2] rounded-full bg-[var(--accent-color)]/15 blur-2xl" />
          <BrandMark className="size-14 rounded-xl shadow-[0_0_0_1px_color-mix(in_srgb,var(--accent-color)_55%,transparent),0_16px_42px_color-mix(in_srgb,var(--accent-color)_28%,transparent)]" />
        </div>
        <div className="mb-5 text-center">
          <p className="font-heading text-lg font-semibold tracking-[-0.035em] text-foreground">YssBI</p>
          <p className="mt-1 font-mono text-[9px] uppercase tracking-[0.18em] text-muted-foreground">
            {t("aboutModal.description")}
          </p>
        </div>

        <Card className="w-full rounded-xl border-[var(--strong-border)] bg-[var(--surface-raised)]/88 shadow-[0_18px_54px_rgb(2_6_23/0.16)] backdrop-blur-sm">
          <CardContent className="flex flex-wrap gap-1.5 p-1.5">
            <Button
              type="button"
              variant="ghost"
              className="h-20 min-w-[160px] flex-1 justify-start gap-3 rounded-lg px-3 text-left hover:bg-[var(--interactive-hover)]"
              onClick={() => addEvent(undefined, { openAfterCreate: true })}
            >
              <span className="flex size-8 shrink-0 items-center justify-center rounded-md bg-[var(--accent-color)] text-white shadow-sm">
                <VscNewFile size={16} />
              </span>
              <span className="flex min-w-0 flex-col items-start gap-0.5">
                <span className="font-heading text-xs font-semibold text-foreground">{t("canvas.newEventGraph")}</span>
                <span className="text-[10px] font-normal text-muted-foreground">{t("canvas.coreLogic")}</span>
              </span>
            </Button>
            <Button
              type="button"
              variant="ghost"
              className="h-20 min-w-[160px] flex-1 justify-start gap-3 rounded-lg px-3 text-left hover:bg-[var(--interactive-hover)]"
              onClick={() => addFunction(undefined, { openAfterCreate: true })}
            >
              <span className="flex size-8 shrink-0 items-center justify-center rounded-md border border-[var(--strong-border)] bg-[var(--surface-sunken)] text-[var(--accent-color)]">
                <VscSymbolMethod size={16} />
              </span>
              <span className="flex min-w-0 flex-col items-start gap-0.5">
                <span className="font-heading text-xs font-semibold text-foreground">{t("canvas.newFunction")}</span>
                <span className="text-[10px] font-normal text-muted-foreground">{t("canvas.reusableRoutine")}</span>
              </span>
            </Button>
            <Button
              type="button"
              variant="ghost"
              className="h-20 min-w-[160px] flex-1 justify-start gap-3 rounded-lg px-3 text-left hover:bg-[var(--interactive-hover)]"
              onClick={() => importGraph()}
            >
              <span className="flex size-8 shrink-0 items-center justify-center rounded-md border border-[var(--strong-border)] bg-[var(--surface-sunken)] text-muted-foreground">
                <VscFolderOpened size={16} />
              </span>
              <span className="flex min-w-0 flex-col items-start gap-1">
                <span className="font-heading text-xs font-semibold text-foreground">{t("canvas.openFile")}</span>
                <span className="flex gap-1">
                  <kbd className="rounded border border-[var(--strong-border)] bg-[var(--surface-sunken)] px-1.5 py-0.5 font-mono text-[9px] font-normal text-muted-foreground">Ctrl</kbd>
                  <kbd className="rounded border border-[var(--strong-border)] bg-[var(--surface-sunken)] px-1.5 py-0.5 font-mono text-[9px] font-normal text-muted-foreground">O</kbd>
                </span>
              </span>
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
};
