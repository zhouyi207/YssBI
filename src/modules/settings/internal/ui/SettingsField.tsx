import type { ReactNode } from "react";
import { Label } from "@/components/ui/label";

export function SettingsField({
  htmlFor,
  label,
  description,
  children,
}: {
  htmlFor: string;
  label: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <div className="settings-field">
      <div className="settings-field-info">
        <Label htmlFor={htmlFor} className="text-sm leading-snug">
          {label}
        </Label>
        {description && (
          <p
            id={`${htmlFor}-description`}
            className="text-xs leading-relaxed text-muted-foreground"
          >
            {description}
          </p>
        )}
      </div>
      <div className="settings-control">{children}</div>
    </div>
  );
}
