import { addGlobalEventListener } from '@/shared/utils/globalEvent';

/**
 * VS Code `GlobalWindowDraggedOverTracker` / `isWindowDraggedOver()`.
 * Passive listeners only — never calls `preventDefault` on dragover.
 */
class GlobalWindowDraggedOverTracker {
  private draggedOver = false;
  private installs = 0;
  private cleanup: (() => void) | null = null;

  get isDraggedOver(): boolean {
    return this.draggedOver;
  }

  install(): () => void {
    this.installs += 1;
    if (this.installs === 1) {
      const onDragOver = () => {
        this.draggedOver = true;
      };
      const onDragLeave = () => {
        this.draggedOver = false;
      };

      const cleanDragOver = addGlobalEventListener(window, 'dragover', onDragOver, true);
      const cleanDragLeave = addGlobalEventListener(window, 'dragleave', onDragLeave, true);
      this.cleanup = () => {
        cleanDragOver();
        cleanDragLeave();
        this.draggedOver = false;
      };
    }

    return () => {
      this.installs = Math.max(0, this.installs - 1);
      if (this.installs === 0 && this.cleanup) {
        this.cleanup();
        this.cleanup = null;
      }
    };
  }
}

const tracker = new GlobalWindowDraggedOverTracker();

export function isWindowDraggedOver(): boolean {
  return tracker.isDraggedOver;
}

export function installGlobalWindowDraggedOverTracker(): () => void {
  return tracker.install();
}
