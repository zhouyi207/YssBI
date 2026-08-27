export interface ResultQueryReader<T> {
  readonly read: (resultId: string) => Promise<T>;
}

export class ResultQueryCoordinator<T> {
  private generation = 0;

  constructor(private readonly reader: ResultQueryReader<T>) {}

  async read(resultId: string): Promise<{ readonly generation: number; readonly result: T }> {
    const generation = ++this.generation;
    const result = await this.reader.read(resultId);
    return { generation, result };
  }

  isCurrent(generation: number): boolean {
    return this.generation === generation;
  }
}
