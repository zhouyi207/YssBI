import { isTauri } from "@tauri-apps/api/core";
import { LogicalSize, PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";
import { availableMonitors, getCurrentWindow } from "@tauri-apps/api/window";

type Page = "projects" | "editor";
const DEFAULT_SIZES = {
  projects: { width: 1100, height: 720 },
  editor: { width: 1600, height: 900 },
} satisfies Record<Page, { width: number; height: number }>;

interface Geometry {
  x: number;
  y: number;
  width: number;
  height: number;
  maximized: boolean;
}

const storageKey = (page: Page) => `yssbi-main-window:${page}`;
let activePage: Page | undefined;
let geometry: Geometry | undefined;
let queue = Promise.resolve();
let listening = false;
let switching = 0;
let capturePending = false;
let captureRequested = false;
let disposed = false;
const unlisten: (() => void)[] = [];

function read(page: Page): Geometry | undefined {
  try {
    const value = JSON.parse(localStorage.getItem(storageKey(page)) ?? "null");
    if (
      value &&
      [value.x, value.y, value.width, value.height].every(Number.isFinite) &&
      value.width > 0 &&
      value.height > 0 &&
      typeof value.maximized === "boolean"
    )
      return value;
  } catch {
    // Invalid or unavailable preferences fall back to the creation defaults.
  }
}

async function capture(): Promise<void> {
  if (disposed || !activePage) return;
  const native = getCurrentWindow();
  if ((await native.isMinimized()) || disposed) return;
  const maximized = await native.isMaximized();
  if (disposed) return;
  if (!maximized) {
    const [position, size] = await Promise.all([native.outerPosition(), native.innerSize()]);
    if (disposed) return;
    geometry = { x: position.x, y: position.y, width: size.width, height: size.height, maximized };
  } else if (geometry) {
    geometry = { ...geometry, maximized };
  }
  if (geometry) {
    try {
      localStorage.setItem(storageKey(activePage), JSON.stringify(geometry));
    } catch {
      console.warn("Main window geometry preferences could not be saved");
    }
  }
}

function enqueue(action: () => Promise<void>): Promise<void> {
  queue = queue.then(action).catch(() => {
    console.warn("Main window geometry could not be saved or restored");
  });
  return queue;
}

/** One native main window, with independent preferences for its two page roles. */
export function restoreMainWindowPage(page: Page): Promise<void> {
  if (disposed || !isTauri() || getCurrentWindow().label !== "main") return Promise.resolve();
  switching += 1;
  return enqueue(async () => {
    try {
      if (disposed) return;
      const native = getCurrentWindow();
      if (!listening) {
        const changed = () => {
          if (disposed || switching !== 0) return;
          captureRequested = true;
          if (capturePending) return;
          capturePending = true;
          void enqueue(async () => {
            try {
              do {
                captureRequested = false;
                await capture();
              } while (captureRequested && switching === 0);
            } finally {
              capturePending = false;
            }
          });
        };
        const stopMoved = await native.onMoved(changed);
        if (disposed) {
          stopMoved();
          return;
        }
        unlisten.push(stopMoved);
        try {
          const stopResized = await native.onResized(changed);
          if (disposed) {
            stopResized();
            return;
          }
          unlisten.push(stopResized);
        } catch (error) {
          for (const stop of unlisten.splice(0)) stop();
          throw error;
        }
        listening = true;
      }
      if (activePage === page) return;
      try {
        await capture();
      } finally {
        // A partial restore must not capture native geometry for either page.
        activePage = undefined;
        geometry = undefined;
      }
      if (disposed) return;
      const saved = read(page);
      await native.unmaximize();
      if (disposed) return;
      if (saved) {
        await native.setSize(new PhysicalSize(saved.width, saved.height));
        if (disposed) return;
        const monitors = await availableMonitors();
        if (disposed) return;
        const visible = monitors.some(
          ({ position, size }) =>
            saved.x + saved.width > position.x &&
            saved.x < position.x + size.width &&
            saved.y >= position.y &&
            saved.y + 40 < position.y + size.height,
        );
        if (visible) await native.setPosition(new PhysicalPosition(saved.x, saved.y));
        else await native.center();
        if (disposed) return;
        if (saved.maximized) await native.maximize();
      } else {
        const { width, height } = DEFAULT_SIZES[page];
        await native.setSize(new LogicalSize(width, height));
        if (disposed) return;
        await native.center();
      }
      if (disposed) return;
      geometry = saved;
      activePage = page;
      await capture();
    } finally {
      switching -= 1;
    }
  });
}

export function disposeMainWindowGeometryForHmr(): void {
  disposed = true;
  activePage = undefined;
  geometry = undefined;
  for (const stop of unlisten.splice(0)) stop();
}

if (import.meta.hot) {
  import.meta.hot.dispose(disposeMainWindowGeometryForHmr);
}
