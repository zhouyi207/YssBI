import { useState, type ReactNode } from "react";

export function Section({
  title,
  collapsible,
  children,
}: {
  title: string;
  collapsible: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  if (collapsible)
    return (
      <details
        className="my-5 rounded-lg border border-border p-4"
        onToggle={(event) => setOpen(event.currentTarget.open)}
      >
        <summary className="cursor-pointer text-sm font-medium">{title}</summary>
        {open && <div className="mt-4">{children}</div>}
      </details>
    );
  return (
    <section className="min-w-0">
      <div className="mb-3 mt-6 flex items-center gap-2 first:mt-0">
        <h3 className="text-sm font-semibold uppercase tracking-wider text-foreground">{title}</h3>
        <div className="ml-2 h-px flex-1 bg-border" />
      </div>
      {children}
    </section>
  );
}
