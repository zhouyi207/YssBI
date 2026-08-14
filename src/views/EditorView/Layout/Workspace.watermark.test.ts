import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const workspaceSource = readFileSync(
  fileURLToPath(new URL('./Workspace.tsx', import.meta.url)),
  'utf8',
);

describe('Workspace empty editor state', () => {
  it('uses WatermarkView as the Dockview watermark', () => {
    expect(workspaceSource).toContain("import { WatermarkView } from '../Canvas/overlays/WatermarkView'");
    expect(workspaceSource).toContain('watermarkComponent={DockviewEditorWatermark}');
    expect(workspaceSource).toContain('return <WatermarkView />');
  });
});
