import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  VscDatabase,
  VscClose,
  VscFile,
  VscTable,
  VscCloudDownload,
  VscChevronRight,
  VscBeaker,
} from "react-icons/vsc";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import type {
  ImportDataSourceType,
  ImportDialogOptions,
} from "@/features/application/ui/applicationUi";
import { ScrollArea } from "@/components/ui/scroll-area";
import { SampleDatasetList } from "./SampleDatasetList";

type CategoryId = "file" | "sql" | "samples" | "other";

type ImportTypeConfig = {
  id: ImportDataSourceType;
  icon: React.ReactNode;
  comingSoon: boolean;
};

const CATEGORIES: { id: CategoryId; icon: React.ReactNode }[] = [
  {
    id: "file",
    icon: <VscFile aria-hidden="true" className="size-3.5" />,
  },
  {
    id: "sql",
    icon: <VscDatabase aria-hidden="true" className="size-3.5" />,
  },
  {
    id: "samples",
    icon: <VscBeaker aria-hidden="true" className="size-3.5" />,
  },
  {
    id: "other",
    icon: <VscCloudDownload aria-hidden="true" className="size-3.5" />,
  },
];

const FILE_TYPES: ImportTypeConfig[] = [
  {
    id: "csv",
    icon: <VscFile aria-hidden="true" className="size-4" />,
    comingSoon: false,
  },
  {
    id: "xlsx",
    icon: <VscTable aria-hidden="true" className="size-4" />,
    comingSoon: false,
  },
];

const SQL_TYPES: ImportTypeConfig[] = [
  {
    id: "sqlite",
    icon: <VscDatabase aria-hidden="true" className="size-4" />,
    comingSoon: false,
  },
  {
    id: "postgres",
    icon: <VscDatabase aria-hidden="true" className="size-4" />,
    comingSoon: false,
  },
  {
    id: "mysql",
    icon: <VscDatabase aria-hidden="true" className="size-4" />,
    comingSoon: false,
  },
  {
    id: "mariadb",
    icon: <VscDatabase aria-hidden="true" className="size-4" />,
    comingSoon: false,
  },
];

const OTHER_TYPES: ImportTypeConfig[] = [
  {
    id: "api",
    icon: <VscCloudDownload aria-hidden="true" className="size-4" />,
    comingSoon: true,
  },
];

const CATEGORY_TYPES: Record<Exclude<CategoryId, "samples">, ImportTypeConfig[]> = {
  file: FILE_TYPES,
  sql: SQL_TYPES,
  other: OTHER_TYPES,
};

function TypeOption({
  type,
  onSelect,
}: {
  type: ImportTypeConfig;
  onSelect: (id: ImportDataSourceType) => void;
}) {
  const { t } = useTranslation();
  const label = t(`importModal.types.${type.id}.label`);
  const description = t(`importModal.types.${type.id}.description`);

  return (
    <Button
      type="button"
      variant="ghost"
      disabled={type.comingSoon}
      onClick={() => {
        if (type.comingSoon) return;
        onSelect(type.id);
      }}
      className="group h-auto min-h-[90px] w-full justify-start gap-4 whitespace-normal rounded-none border-0 border-b border-border px-0 py-5 text-left font-normal focus-visible:ring-inset disabled:opacity-60"
    >
      <span className="flex min-w-0 flex-1 items-start gap-3">
        <span className="mt-0.5 shrink-0 text-muted-foreground">{type.icon}</span>
        <span className="min-w-0 flex-1">
          <span className="block text-[13px] leading-normal text-foreground">{label}</span>
          <span className="mt-1 block text-xs leading-[1.65] text-muted-foreground">
            {description}
          </span>
        </span>
      </span>
      {type.comingSoon ? (
        <Badge
          variant="outline"
          className="shrink-0 rounded-sm px-1.5 text-[10px] font-normal text-muted-foreground"
        >
          {t("importModal.developing")}
        </Badge>
      ) : (
        <VscChevronRight
          aria-hidden="true"
          className="size-4 text-muted-foreground group-hover:text-foreground"
        />
      )}
    </Button>
  );
}

export const ImportModal = ({
  options,
  onClose,
}: {
  options: ImportDialogOptions;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const sectionHeadingId = useId();
  const [selectedCategory, setSelectedCategory] = useState<CategoryId>("file");
  const [importingSample, setImportingSample] = useState(false);
  const types = selectedCategory === "samples" ? [] : CATEGORY_TYPES[selectedCategory];

  return (
    <Dialog open onOpenChange={(open) => !open && !importingSample && onClose()}>
      <DialogContent className="flex h-[min(720px,88dvh)] max-w-[min(1000px,92vw)] flex-col gap-0 rounded-md bg-[var(--workbench-bg)] p-0 motion-reduce:animate-none max-[720px]:h-[92dvh] max-[720px]:max-w-[96vw]">
        <div className="flex h-9 shrink-0 items-center justify-between gap-4 border-b border-border bg-[var(--sidebar-bg)] pl-3.5 pr-2.5">
          <DialogTitle className="flex min-w-0 items-center gap-2 text-xs font-medium normal-case tracking-normal">
            <VscDatabase aria-hidden="true" className="size-3.5 shrink-0 text-muted-foreground" />
            <span className="truncate">{t("importModal.title")}</span>
          </DialogTitle>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            onClick={onClose}
            disabled={importingSample}
            aria-label={t("importModal.close")}
            className="h-[26px] w-7 shrink-0 rounded-sm text-muted-foreground"
          >
            <VscClose aria-hidden="true" className="size-3.5" />
          </Button>
        </div>

        <div className="flex min-h-0 flex-1 overflow-hidden max-[720px]:flex-col">
          <nav
            aria-label={t("importModal.title")}
            className="flex w-56 shrink-0 flex-col gap-0.5 border-r border-border bg-[var(--sidebar-bg)] px-2.5 py-3 min-[721px]:max-[900px]:w-[200px] max-[720px]:w-full max-[720px]:flex-row max-[720px]:overflow-x-auto max-[720px]:border-b max-[720px]:border-r-0 max-[720px]:px-3"
          >
            {CATEGORIES.map((cat) => {
              const active = selectedCategory === cat.id;
              return (
                <Button
                  key={cat.id}
                  type="button"
                  variant="ghost"
                  onClick={() => setSelectedCategory(cat.id)}
                  disabled={importingSample}
                  aria-current={active ? "page" : undefined}
                  className={cn(
                    "h-7 w-full justify-start gap-2.5 rounded-sm px-2.5 text-[13px] font-normal text-muted-foreground max-[720px]:w-auto",
                    active &&
                      "border-[color-mix(in_srgb,var(--foreground)_8%,transparent)] bg-[color-mix(in_srgb,var(--foreground)_9%,transparent)] text-foreground hover:bg-[color-mix(in_srgb,var(--foreground)_9%,transparent)] dark:hover:bg-[color-mix(in_srgb,var(--foreground)_9%,transparent)]",
                  )}
                >
                  {cat.icon}
                  <span className="truncate">{t(`importModal.categories.${cat.id}`)}</span>
                </Button>
              );
            })}
          </nav>

          <ScrollArea className="min-h-0 min-w-0 flex-1">
            <section
              aria-labelledby={sectionHeadingId}
              className="w-full max-w-[920px] px-8 pb-10 pt-7 min-[721px]:max-[900px]:px-6 max-[720px]:px-5 max-[720px]:py-6"
            >
              <div className="border-b border-border pb-5">
                <h2 id={sectionHeadingId} className="text-base font-medium leading-normal">
                  {t(`importModal.categories.${selectedCategory}`)}
                </h2>
                <DialogDescription className="mt-3 text-xs leading-[1.6]">
                  {t(
                    selectedCategory === "samples"
                      ? "importModal.samples.subtitle"
                      : "importModal.subtitle",
                  )}
                </DialogDescription>
              </div>
              {selectedCategory === "samples" && (
                <SampleDatasetList
                  onImport={options.onImportSample}
                  onImported={onClose}
                  onBusyChange={setImportingSample}
                />
              )}
              {types.map((type) => (
                <TypeOption key={type.id} type={type} onSelect={options.onSelect} />
              ))}
            </section>
          </ScrollArea>
        </div>
      </DialogContent>
    </Dialog>
  );
};
