/**
 * Pin 偏移等待器。
 *
 * Pin 相对节点原点的偏移（pinOffset）是在节点渲染后由 `useCanvasViewport` 通过 DOM
 * 测量得到的，创建节点时无法同步得知。本模块让创建流程可以 await「某个 pin 的偏移
 * 被测量出来」，从而在测得后把节点位置反向平移，使该 pin 精确落在拖拽释放点。
 *
 * 之所以用测量而非按布局常量推算：节点宽高随标题/pin 文本动态变化，输出 pin 的
 * x 偏移等于节点宽度，按常量推算并不可靠。
 */

export interface PinOffset {
  x: number;
  y: number;
}

interface Waiter {
  graphId: string;
  pinId: string;
  resolve: (offset: PinOffset | null) => void;
  timer: ReturnType<typeof setTimeout>;
}

const waiters: Waiter[] = [];

/**
 * 等待指定 pin 的偏移被测量出来。
 * 超时（节点被裁剪/未渲染等）则以 null 兑现，调用方应回退为不对齐。
 */
export function waitForPinOffset(
  graphId: string,
  pinId: string,
  timeoutMs = 500,
): Promise<PinOffset | null> {
  return new Promise((resolve) => {
    const waiter: Waiter = {
      graphId,
      pinId,
      resolve,
      timer: setTimeout(() => {
        const idx = waiters.indexOf(waiter);
        if (idx >= 0) waiters.splice(idx, 1);
        resolve(null);
      }, timeoutMs),
    };
    waiters.push(waiter);
  });
}

/**
 * 由 `useCanvasViewport` 在每次测量后调用：兑现所有偏移已可用的等待者。
 */
export function resolvePinOffsetWaiters(
  graphId: string,
  offsets: Record<string, PinOffset>,
): void {
  if (waiters.length === 0) return;
  for (let i = waiters.length - 1; i >= 0; i--) {
    const w = waiters[i];
    if (w.graphId !== graphId) continue;
    const offset = offsets[w.pinId];
    if (offset) {
      clearTimeout(w.timer);
      waiters.splice(i, 1);
      w.resolve(offset);
    }
  }
}
