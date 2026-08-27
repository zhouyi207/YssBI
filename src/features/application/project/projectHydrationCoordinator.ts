export interface ProjectHydrationRequest {
  readonly path: string;
  readonly generation: number;
}

export interface ProjectHydrationResult<T> {
  readonly generation: number;
  readonly value: T;
}

export interface ProjectHydrationLoader<T> {
  readonly load: (request: ProjectHydrationRequest) => Promise<T>;
}

export class ProjectHydrationCoordinator<T> {
  private generation = 0;

  constructor(private readonly loader: ProjectHydrationLoader<T>) {}

  async hydrate(path: string): Promise<ProjectHydrationResult<T>> {
    const generation = ++this.generation;
    const value = await this.loader.load({ path, generation });
    return { generation, value };
  }

  isCurrent(generation: number): boolean {
    return this.generation === generation;
  }
}
