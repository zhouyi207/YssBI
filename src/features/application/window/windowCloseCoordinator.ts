export type CloseDecision = "allow" | "prevent";

export interface WindowCloseWorkflow {
  readonly decide: () => CloseDecision;
  readonly close: () => Promise<void>;
}

export class WindowCloseCoordinator {
  constructor(private readonly workflow: WindowCloseWorkflow) {}

  async requestClose(): Promise<CloseDecision> {
    const decision = this.workflow.decide();
    if (decision === "allow") await this.workflow.close();
    return decision;
  }
}
