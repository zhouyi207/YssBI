import type { ProjectPublicationCapability } from '@/features/core/project/publication';
import type { ReadonlyProjectSnapshot } from '@/features/core/project/read';

export interface ProjectEventReceipt {
  readonly projectInstanceId: string;
  readonly publicationRevision: number;
  readonly snapshot: ReadonlyProjectSnapshot;
}

export class ProjectEventReconciler {
  constructor(private readonly publication: ProjectPublicationCapability) {}

  publish(receipt: ProjectEventReceipt): void {
    this.publication.publishProjectSnapshot(receipt.snapshot);
  }
}
