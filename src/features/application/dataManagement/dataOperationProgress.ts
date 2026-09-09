import { uiStore } from "@/features/core/ui/UIStore";

/** 让 ProgressOverlay 先绘制一帧，再开始阻塞 invoke。 */
async function flushProgressFrame() {
  await new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });
}

/** 导入及来源读取等耗时操作：显示全局进度蒙层。资源删除走普通提交反馈。 */
export async function runWithDataOperationProgress<T>(
  stage: string,
  detail: string | undefined,
  run: () => Promise<T>,
): Promise<T> {
  uiStore.startProgress({ stage, detail, cancelable: false });
  await flushProgressFrame();
  try {
    return await run();
  } finally {
    uiStore.finishProgress();
  }
}
