import { useTranslation } from "react-i18next";
import { VscFolderOpened, VscNewFile, VscRefresh } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { ProjectFlowGraphic } from "./ProjectFlowGraphic";

interface ProjectPickerHeroProps {
  isBusy: boolean;
  creating: boolean;
  importing: boolean;
  scanning: boolean;
  onNewProject: () => void;
  onImportProject: () => void;
  onScanProjects: () => void;
}

export function ProjectPickerHero({
  isBusy,
  creating,
  importing,
  scanning,
  onNewProject,
  onImportProject,
  onScanProjects,
}: ProjectPickerHeroProps) {
  const { t } = useTranslation();

  return (
    <section className="project-picker-hero relative shrink-0 overflow-hidden border-b border-[var(--strong-border)] bg-[var(--surface-raised)]/85 px-8 py-7 max-[640px]:px-5 max-[640px]:py-4">
      <div className="relative z-10 max-w-[640px]">
        <div className="mb-2 flex items-center gap-2 font-mono text-[10px] font-semibold uppercase tracking-[0.18em] text-[var(--accent-color)]">
          <span className="h-px w-6 bg-current" />
          {t("projectPicker.title")}
        </div>
        <h1 className="font-heading text-[clamp(1.55rem,2.6vw,2.35rem)] font-semibold leading-tight tracking-[-0.04em] text-foreground">
          {t("projectPicker.heading")}
        </h1>
        <p className="project-picker-hero-description mt-2 max-w-[560px] text-sm leading-6 text-muted-foreground max-[520px]:hidden">
          {t("projectPicker.description")}
        </p>
        <div className="project-picker-hero-actions mt-5 flex flex-wrap items-center gap-2">
          <Button
            type="button"
            size="lg"
            disabled={isBusy}
            onClick={onNewProject}
            className="h-9 gap-2 bg-[var(--accent-color)] px-4 text-primary-foreground shadow-[0_8px_24px_color-mix(in_srgb,var(--accent-color)_22%,transparent)] hover:bg-[var(--accent-color-hover)]"
          >
            <VscNewFile size={15} />
            {creating ? t("projectPicker.creating") : t("projectPicker.newProject")}
          </Button>
          <Button
            type="button"
            variant="outline"
            size="lg"
            disabled={isBusy}
            onClick={onImportProject}
            className="h-9 gap-2 bg-card/70 px-4"
          >
            <VscFolderOpened size={15} />
            {importing ? t("projectPicker.importing") : t("projectPicker.importProject")}
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="lg"
            disabled={isBusy}
            onClick={onScanProjects}
            className="h-9 gap-2 px-3 text-muted-foreground hover:text-foreground"
          >
            <VscRefresh size={15} />
            {scanning ? t("projectPicker.scanning") : t("projectPicker.scanProjects")}
          </Button>
        </div>
      </div>
      <ProjectFlowGraphic className="pointer-events-none absolute -right-7 top-1/2 hidden h-[150%] w-[46%] -translate-y-1/2 opacity-80 xl:block" />
    </section>
  );
}
