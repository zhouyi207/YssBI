import { useTranslation } from "react-i18next";

export function WatermarkView() {
  const { t } = useTranslation();

  return (
    <div className="flex h-full w-full select-none items-center justify-center overflow-hidden bg-[var(--workbench-bg)] px-6 py-10">
      <div className="max-w-[620px] text-center">
        <p className="font-heading text-4xl font-semibold tracking-[-0.035em] text-foreground">
          YssBI
        </p>
        <p className="mt-3 font-sans text-sm leading-relaxed tracking-normal text-muted-foreground">
          {t("aboutModal.description")}
        </p>
      </div>
    </div>
  );
}
