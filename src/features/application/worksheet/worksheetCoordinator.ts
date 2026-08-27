import type { WorksheetReadSnapshot } from '@/features/core/worksheet/read';

export interface WorksheetReader {
  readonly load: (path: string) => Promise<WorksheetReadSnapshot>;
  readonly save: (snapshot: WorksheetReadSnapshot) => Promise<void>;
}

export class WorksheetCoordinator {
  private generation = 0;

  constructor(private readonly reader: WorksheetReader) {}

  async load(path: string): Promise<{ readonly generation: number; readonly worksheet: WorksheetReadSnapshot }> {
    const generation = ++this.generation;
    const worksheet = await this.reader.load(path);
    return { generation, worksheet };
  }

  async save(snapshot: WorksheetReadSnapshot): Promise<number> {
    const generation = ++this.generation;
    await this.reader.save(snapshot);
    return generation;
  }
}
