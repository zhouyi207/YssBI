import {
  useEffect,
  useRef,
  useState,
  type ComponentProps,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { DetailFieldRow } from "./DetailFieldRow";
import { DetailText } from "./DetailText";
import { detailTableClass } from "./detailStyles";

interface DetailFormProps {
  children: ReactNode;
  className?: string;
}

export function DetailForm({ children, className }: DetailFormProps) {
  return (
    <Card
      className={cn(
        "rounded-none border-0 bg-transparent py-0 shadow-none",
        detailTableClass,
        className,
      )}
    >
      <CardContent className="flex flex-col gap-1 px-3 py-1">{children}</CardContent>
    </Card>
  );
}

interface DetailCommitInputProps extends Pick<
  ComponentProps<"input">,
  "aria-invalid" | "aria-describedby"
> {
  value: string;
  onCommit: (value: string) => void | Promise<void>;
  className?: string;
  type?: string;
}

export function DetailTextarea({ className, ...props }: ComponentProps<"textarea">) {
  return (
    <textarea
      {...props}
      className={cn(
        "block min-h-20 w-full resize-y rounded-md border border-border bg-input/30 px-3 py-2 text-left text-sm text-foreground outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30 disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
    />
  );
}

export function DetailCommitInput({
  value,
  onCommit,
  className,
  type = "text",
  ...accessibility
}: DetailCommitInputProps) {
  const [draft, setDraft] = useState(value);
  const skipNextBlurCommitRef = useRef(false);

  useEffect(() => {
    setDraft(value);
  }, [value]);

  const commit = () => {
    if (skipNextBlurCommitRef.current) {
      skipNextBlurCommitRef.current = false;
      return;
    }
    if (draft === value) return;
    void onCommit(draft);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.currentTarget.blur();
      return;
    }
    if (event.key === "Escape") {
      skipNextBlurCommitRef.current = true;
      setDraft(value);
      event.currentTarget.blur();
    }
  };

  return (
    <Input
      {...accessibility}
      className={className}
      type={type}
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={handleKeyDown}
    />
  );
}

interface DetailReadonlyFieldProps {
  label: ReactNode;
  children: ReactNode;
  tone?: "body" | "muted" | "smallMuted" | "mono" | "accentMono";
  className?: string;
  labelClassName?: string;
  valueClassName?: string;
}

export function DetailReadonlyField({
  label,
  children,
  tone = "muted",
  className,
  labelClassName,
  valueClassName,
}: DetailReadonlyFieldProps) {
  return (
    <DetailFieldRow label={label} labelClassName={labelClassName} valueClassName={valueClassName}>
      <DetailText
        as="div"
        tone={tone}
        className={cn(
          "flex min-h-7 min-w-0 items-center justify-end px-2 py-0.5 text-right",
          className,
        )}
      >
        <span
          className="min-w-0 truncate"
          title={typeof children === "string" ? children : undefined}
        >
          {children}
        </span>
      </DetailText>
    </DetailFieldRow>
  );
}
