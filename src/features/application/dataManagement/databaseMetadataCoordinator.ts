import type { DatabaseReadSnapshot } from '@/features/core/database/read';

export interface DatabaseMetadataReader {
  readonly read: (id: string) => Promise<DatabaseReadSnapshot>;
}

export class DatabaseMetadataCoordinator {
  private generation = 0;

  constructor(private readonly reader: DatabaseMetadataReader) {}

  async read(id: string): Promise<{ readonly generation: number; readonly database: DatabaseReadSnapshot }> {
    const generation = ++this.generation;
    const database = await this.reader.read(id);
    return { generation, database };
  }

  isCurrent(generation: number): boolean {
    return this.generation === generation;
  }
}
