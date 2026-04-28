import { invoke } from "@tauri-apps/api/core";

export class PanelDidService {
  static async computeFakeGroupRi<TRequest, TResponse>(req: TRequest): Promise<TResponse> {
    return invoke<TResponse>("compute_panel_did_fake_group_ri", { req });
  }
}
