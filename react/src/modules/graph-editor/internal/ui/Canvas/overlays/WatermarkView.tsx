import { useTranslation } from "react-i18next";

export function WatermarkView() {
  const { t } = useTranslation();

  return (
    <div className="relative isolate flex h-full w-full select-none items-center justify-center overflow-hidden bg-[var(--workbench-bg)] px-6 py-10">
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0"
        style={{
          background:
            "radial-gradient(ellipse 320px 200px at center, color-mix(in srgb, var(--accent-color) 7%, transparent), transparent 75%)",
        }}
      />
      <div className="relative flex max-w-[620px] flex-col items-center text-center">
        <p className="font-heading text-[clamp(2.5rem,5vw,4rem)] font-semibold leading-none tracking-[-0.055em] text-foreground/80">
          Yss<span className="text-[var(--accent-color)]">BI</span>
        </p>
        <div
          aria-hidden="true"
          className="my-5 h-px w-16"
          style={{
            background:
              "linear-gradient(to right, transparent, color-mix(in srgb, var(--accent-color) 45%, transparent), transparent)",
          }}
        />
        <p className="text-balance font-sans text-sm leading-relaxed tracking-normal text-muted-foreground">
          {t("aboutModal.description")}
        </p>
      </div>
    </div>
  );
}
