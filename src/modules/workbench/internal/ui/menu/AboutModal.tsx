import { useTranslation } from "react-i18next";
import { VscGithub } from "react-icons/vsc";
import { APP_DISPLAY_NAME, APP_VERSION } from "@/shared/appLinks";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

interface AboutModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onOpenRepository: () => void;
  onReportIssue: () => void;
}

export function AboutModal({
  open,
  onOpenChange,
  onOpenRepository,
  onReportIssue,
}: AboutModalProps) {
  const { t } = useTranslation();

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("aboutModal.title", { appName: APP_DISPLAY_NAME })}</DialogTitle>
        </DialogHeader>

        <div className="flex flex-col items-center gap-4 py-2 text-center">
          <div className="flex size-14 items-center justify-center rounded-xl bg-[var(--accent-color)]">
            <span className="text-2xl font-black text-white">Y</span>
          </div>
          <div className="space-y-1">
            <p className="text-lg font-semibold text-foreground">{APP_DISPLAY_NAME}</p>
            <p className="text-sm text-muted-foreground">
              {t("aboutModal.version", { version: APP_VERSION })}
            </p>
          </div>
          <p className="max-w-sm text-sm leading-relaxed text-muted-foreground">
            {t("aboutModal.description")}
          </p>
        </div>

        <DialogFooter className="flex-col gap-2 sm:flex-col sm:space-x-0">
          <Button
            type="button"
            variant="outline"
            className="w-full justify-center gap-2"
            onClick={onOpenRepository}
          >
            <VscGithub size={16} />
            {t("menubar.githubRepository")}
          </Button>
          <Button type="button" variant="outline" className="w-full" onClick={onReportIssue}>
            {t("menubar.reportIssue")}
          </Button>
          <Button type="button" className="w-full" onClick={() => onOpenChange(false)}>
            {t("common.close")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
