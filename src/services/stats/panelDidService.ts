import { invokeCommand } from "@/services/ipc";

export class PanelDidService {
  static async computeFakeGroupRi<TRequest, TResponse>(req: TRequest): Promise<TResponse> {
    return invokeCommand<TResponse>("compute_panel_did_fake_group_ri", { req });
  }
}
