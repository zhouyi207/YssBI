import { useTranslation } from "react-i18next";
import { useEditorSessionCommandsContext } from "@/features/application/editor";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";

export const WatermarkView = () => {
  const { t } = useTranslation();
  const { addEvent, addFunction, importGraph } = useEditorSessionCommandsContext();

  return (
    <div className="relative w-full h-full flex flex-col items-center justify-center bg-[var(--workbench-bg)] select-none overflow-hidden">
      {/* Simplified Logo */}
      <div className="mb-8 opacity-20 group">
        <svg className="w-32 h-32 text-foreground" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1">
          <path strokeLinecap="round" strokeLinejoin="round" d="M11 3.055A9.001 9.001 0 1020.945 13H11V3.055z" />
          <path strokeLinecap="round" strokeLinejoin="round" d="M20.488 9H15V3.512A9.025 9.025 0 0120.488 9z" />
        </svg>
      </div>
      {/* Shortcut Hints */}
      <Card className="min-w-[360px] bg-card/60 backdrop-blur-sm">
        <CardContent className="flex flex-col gap-2 p-2">
          <Button type="button" variant="ghost" className="h-auto justify-between gap-12 p-2" onClick={() => addEvent(undefined, { openAfterCreate: true })}>
            <span className="flex items-center gap-2">
              <span className="w-1.5 h-1.5 rounded-full bg-red-500" />
              <span>{t("canvas.newEventGraph")}</span>
            </span>
            <span className="text-[10px] text-muted-foreground italic">{t("canvas.coreLogic")}</span>
          </Button>
          <Button type="button" variant="ghost" className="h-auto justify-between gap-12 p-2" onClick={() => addFunction(undefined, { openAfterCreate: true })}>
            <span className="flex items-center gap-2">
              <span className="w-1.5 h-1.5 rounded-full bg-blue-500" />
              <span>{t("canvas.newFunction")}</span>
            </span>
            <span className="text-[10px] text-muted-foreground italic">{t("canvas.reusableRoutine")}</span>
          </Button>
          <Button type="button" variant="ghost" className="h-auto justify-between gap-12 p-2" onClick={() => importGraph()}>
            <span>{t("canvas.openFile")}</span>
            <span className="flex gap-1">
              <kbd className="rounded border border-border bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">Ctrl</kbd>
              <kbd className="rounded border border-border bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">O</kbd>
            </span>
          </Button>
          <Button type="button" variant="ghost" disabled className="h-auto justify-between gap-12 p-2">
            <span>{t("canvas.showAllCommands")}</span>
            <span className="flex gap-1">
              <kbd className="rounded border border-border bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">Ctrl</kbd>
              <kbd className="rounded border border-border bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">Shift</kbd>
              <kbd className="rounded border border-border bg-muted px-1.5 py-0.5 text-[10px] text-muted-foreground">P</kbd>
            </span>
          </Button>
        </CardContent>
      </Card>
      {/* Subtle grid background for the empty state too, but very faint */}
      <div className="absolute inset-0 opacity-[0.03] pointer-events-none"
        style={{
          backgroundImage: `linear-gradient(var(--foreground) 1px, transparent 1px), linear-gradient(90deg, var(--foreground) 1px, transparent 1px)`,
          backgroundSize: '40px 40px'
        }}
      />
    </div>
  );
};
