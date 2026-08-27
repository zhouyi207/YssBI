import type { ExecutionReadSnapshot } from '@/features/core/execution/read';
import type { ExecutionPublicationCapability } from '@/features/core/execution/publication';

export class ExecutionProjectionCoordinator {
  constructor(private readonly publication: ExecutionPublicationCapability) {}

  publish(snapshot: ExecutionReadSnapshot): void {
    this.publication.publishExecution(snapshot);
  }

  reset(): void {
    this.publication.resetExecution();
  }
}
