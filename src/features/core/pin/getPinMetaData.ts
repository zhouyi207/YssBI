/**
 * 从节点定义中获取指定 pin 的 metaData（如 dropdown 的 widget_options）
 * 用于渲染 Pin 时根据 schema 显示下拉框等控件
 */

import type { NodeDefinition, PinMetaDataDTO, PinSlot } from "@/shared/types/domain";

function getPinDefFromSlot(slot: PinSlot): { name: string; namePrefix?: string; metaData: PinMetaDataDTO } | null {
  if (slot.slotKind === "fixed") {
    return {
      name: slot.pin.name,
      metaData: slot.pin.metaData,
    };
  }
  if (slot.slotKind === "repeatable") {
    return {
      name: slot.template.name,
      namePrefix: slot.namePrefix,
      metaData: slot.template.metaData,
    };
  }
  return null;
}

/**
 * 根据 nodeType 和 pin 的 name 查找对应的 metaData
 */
export function getPinMetaData(
  definition: NodeDefinition | undefined,
  pinName: string
): PinMetaDataDTO | undefined {
  if (!definition?.pinSlots) return undefined;

  for (const slot of definition.pinSlots) {
    const pinDef = getPinDefFromSlot(slot);
    if (!pinDef) continue;

    if (slot.slotKind === "fixed" && pinDef.name === pinName) {
      return pinDef.metaData;
    }
    if (slot.slotKind === "repeatable" && pinDef.namePrefix && pinName.startsWith(pinDef.namePrefix)) {
      return pinDef.metaData;
    }
  }
  return undefined;
}
