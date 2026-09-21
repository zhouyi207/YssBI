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
  if (!activePage) return;
  const native = getCurrentWindow();
  if (await native.isMinimized()) return;
  const maximized = await native.isMaximized();
  if (!maximized) {
    const [position, size] = await Promise.all([native.outerPosition(), native.innerSize()]);
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
  if (!isTauri() || getCurrentWindow().label !== "main") return Promise.resolve();
  switching += 1;
  return enqueue(async () => {
    try {
      const native = getCurrentWindow();
      if (!listening) {
        const changed = () => {
          if (switching !== 0) return;
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
        unlisten.push(await native.onMoved(changed));
        unlisten.push(await native.onResized(changed));
        listening = true;
      }
      if (activePage === page) return;
      await capture();
      activePage = page;
      geometry = read(page);
      await native.unmaximize();
      if (geometry) {
        await native.setSize(new PhysicalSize(geometry.width, geometry.height));
        const saved = geometry;
        const monitors = await availableMonitors();
        const visible = monitors.some(
          ({ position, size }) =>
            saved.x + saved.width > position.x &&
            saved.x < position.x + size.width &&
            saved.y >= position.y &&
            saved.y + 40 < position.y + size.height,
        );
        if (visible) await native.setPosition(new PhysicalPosition(saved.x, saved.y));
        else await native.center();
        if (saved.maximized) await native.maximize();
      } else {
        const { width, height } = DEFAULT_SIZES[page];
        await native.setSize(new LogicalSize(width, height));
        await native.center();
      }
      await capture();
    } finally {
      switching -= 1;
    }
  });
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => unlisten.forEach((stop) => stop()));
}
