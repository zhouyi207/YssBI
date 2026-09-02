import { invokeCommand } from "@/services/ipc";
import {
  parseApplicationSettingsMutationReceipt,
  parseApplicationSettingsSnapshot,
  type ApplicationSettingsMutationReceiptDto,
  type ApplicationSettingsMutationRequestDto,
  type ApplicationSettingsSnapshotDto,
} from "@/shared/types/dto/applicationSettings";

export class ApplicationSettingsService {
  static async get(): Promise<ApplicationSettingsSnapshotDto> {
    return parseApplicationSettingsSnapshot(await invokeCommand("get_application_settings"));
  }

  static async update(
    request: ApplicationSettingsMutationRequestDto,
  ): Promise<ApplicationSettingsMutationReceiptDto> {
    return parseApplicationSettingsMutationReceipt(
      await invokeCommand("update_application_settings", { request }),
    );
  }
}
