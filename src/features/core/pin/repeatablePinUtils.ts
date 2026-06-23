import type { Pin } from "@/shared/types/domain";
import type { NodeDefinition, PinSlot } from "@/shared/types/domain/node";

export type RepeatablePinSlot = Extract<PinSlot, { slotKind: "repeatable" }>;

export function getRepeatableSlot(definition: NodeDefinition | undefined): RepeatablePinSlot | null {
  if (!definition?.pinSlots) return null;
  const slot = definition.pinSlots.find((s) => s.slotKind === "repeatable");
  return slot?.slotKind === "repeatable" ? slot : null;
}

function fixedPinNames(definition: NodeDefinition): Set<string> {
  const names = new Set<string>();
  for (const slot of definition.pinSlots) {
    if (slot.slotKind === "fixed") {
      names.add(slot.pin.name);
    }
  }
  return names;
}

/** Mirrors backend `generate_slot_name`: empty prefix → A..Z or "Pin N"; else "{prefix} {index+1}" */
export function matchesRepeatableSlotName(pinName: string, namePrefix: string): boolean {
  if (namePrefix) {
    const escaped = namePrefix.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return new RegExp(`^${escaped} \\d+$`).test(pinName);
  }
  return /^[A-Z]$/.test(pinName) || /^Pin \d+$/.test(pinName);
}

export function findRepeatableSlotForPin(
  pin: Pin,
  definition: NodeDefinition | undefined
): RepeatablePinSlot | null {
  if (!definition?.pinSlots) return null;
  if (fixedPinNames(definition).has(pin.name)) return null;

  for (const slot of definition.pinSlots) {
    if (slot.slotKind !== "repeatable") continue;
    if (pin.direction !== slot.template.direction) continue;
    if (matchesRepeatableSlotName(pin.name, slot.namePrefix)) return slot;
  }
  return null;
}

/** Whether a pin instance belongs to any repeatable slot on the node */
export function isRepeatableSlotPin(pin: Pin, definition: NodeDefinition | undefined): boolean {
  return findRepeatableSlotForPin(pin, definition) != null;
}

export function countPinsInRepeatableSlot(
  pins: Pin[],
  slot: RepeatablePinSlot,
  definition: NodeDefinition
): number {
  return pins.filter(
    (p) =>
      p.direction === slot.template.direction &&
      !fixedPinNames(definition).has(p.name) &&
      matchesRepeatableSlotName(p.name, slot.namePrefix)
  ).length;
}

export function countRepeatableSlotPins(pins: Pin[], definition: NodeDefinition | undefined): number {
  const slot = getRepeatableSlot(definition);
  if (!slot || !definition) return 0;
  return countPinsInRepeatableSlot(pins, slot, definition);
}

export function canRemoveRepeatablePin(
  pin: Pin,
  definition: NodeDefinition | undefined,
  pinsOnNode: Pin[]
): boolean {
  const slot = findRepeatableSlotForPin(pin, definition);
  if (!slot || !definition) return false;
  const count = countPinsInRepeatableSlot(pinsOnNode, slot, definition);
  return count > slot.minCount;
}
