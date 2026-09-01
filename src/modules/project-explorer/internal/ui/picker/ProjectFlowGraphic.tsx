import { cn } from "@/lib/utils";

export function ProjectFlowGraphic({ className }: { className?: string }) {
  return (
    <svg
      aria-hidden="true"
      viewBox="0 0 420 180"
      fill="none"
      className={cn("text-[var(--accent-color)]", className)}
    >
      <path
        d="M24 48h70c28 0 34 34 62 34h54"
        stroke="currentColor"
        strokeOpacity="0.18"
        strokeWidth="1.25"
      />
      <path
        d="M24 132h70c28 0 34-34 62-34h54"
        stroke="currentColor"
        strokeOpacity="0.18"
        strokeWidth="1.25"
      />
      <path
        d="M210 82h48c34 0 36-46 70-46h68"
        stroke="currentColor"
        strokeOpacity="0.18"
        strokeWidth="1.25"
      />
      <path
        d="M210 98h48c34 0 36 46 70 46h68"
        stroke="currentColor"
        strokeOpacity="0.18"
        strokeWidth="1.25"
      />
      <path
        className="project-flow-signal"
        d="M24 48h70c28 0 34 34 62 34h102c34 0 36 62 70 62h68"
        stroke="currentColor"
        strokeOpacity="0.72"
        strokeWidth="1.5"
      />
      <g fill="var(--surface-raised)" stroke="currentColor" strokeWidth="1.5">
        <circle cx="24" cy="48" r="5" />
        <circle cx="24" cy="132" r="5" />
        <circle cx="210" cy="90" r="7" />
        <circle cx="396" cy="36" r="5" />
        <circle cx="396" cy="144" r="5" />
      </g>
      <circle cx="210" cy="90" r="2.5" fill="currentColor" />
    </svg>
  );
}
