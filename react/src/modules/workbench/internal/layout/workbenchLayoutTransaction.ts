import { configureWorkbenchModel } from "./workbenchActivityGroup";
import { Model } from "flexlayout-react";
import { LayoutModelBinding } from "./layoutModelBinding";
import { WorkbenchModelOperations } from "./workbenchLayoutOperations";
import { isValidRootLayout } from "./workbenchLayoutPersistence";
import { WorkbenchLayoutError } from "./workbenchTypes";

/** Speculative changes use the library's own model; only validated candidates adopt live views. */
export class PendingWorkbenchTransaction {
  readonly operations: WorkbenchModelOperations;
  private readonly baseRevision: number;
  constructor(private readonly binding: LayoutModelBinding) {
    this.baseRevision = binding.getSnapshot().revision;
    this.operations = new WorkbenchModelOperations(
      Model.fromJson(structuredClone(binding.getModel().toJson())),
    );
    configureWorkbenchModel(this.operations.model);
  }
  commit(): void {
    if (this.binding.getSnapshot().revision !== this.baseRevision)
      throw new WorkbenchLayoutError("layout_restore_failed", { reason: "stale_transaction" });
    const candidate = this.operations.serialize();
    if (!isValidRootLayout(candidate))
      throw new WorkbenchLayoutError("layout_restore_failed", { reason: "invalid_layout" });
    if (JSON.stringify(candidate) !== JSON.stringify(this.binding.getModel().toJson()))
      this.binding.replace(candidate);
  }
}
