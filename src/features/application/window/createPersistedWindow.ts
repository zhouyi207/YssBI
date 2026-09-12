import { createWebviewWindow } from "@/services/platform/webviewWindow";
import { readWindowDecorationsFromSettings } from "@/features/application/window/windowDecorationPolicy";

// Creation defaults are logical pixels. The Rust plugin restores saved physical geometry.
const DEFAULT_SIZES = {
  dataview: { width: 1000, height: 600 },
  logs: { width: 1000, height: 600 },
  inspect: { width: 1000, height: 600 },
  plot: { width: 960, height: 800 },
  info: { width: 960, height: 800 },
} as const;

export type WindowKind = keyof typeof DEFAULT_SIZES;

export interface PersistedWindowOptions {
  kind: WindowKind;
  label: string;
  url: string;
  title: string;
  /** 是否显示原生装饰；默认读取 appearance.titleBarStyle */
  decorations?: boolean;
}

/** Create hidden; the native plugin restores geometry and the ready view reveals itself. */
export async function createPersistedWindow(opts: PersistedWindowOptions): Promise<void> {
  const result = await createWebviewWindow({
    label: opts.label,
    url: opts.url,
    title: opts.title,
    ...DEFAULT_SIZES[opts.kind],
    decorations: opts.decorations ?? readWindowDecorationsFromSettings(),
    visible: false,
  });
  if (!result.ok) throw new Error(result.failure.code);
}
