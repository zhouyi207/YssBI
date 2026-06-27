import type { PinView } from '@/shared/types/store/graph';
import type { NodeDefinition, PinDefinitionDTO, PinSlot } from '@/shared/types/domain/node';

export interface ResolvedPinSpec {
  id: string;
  name: string;
  direction: 'input' | 'output';
  kind: 'Data' | 'Exec';
  type: string;
  typeDisplay?: string;
  optional: boolean;
  slotKind?: 'fixed' | 'repeatable' | 'derivedFromInput';
  slotNote?: { kind: 'repeatableRange'; min: number; max: number | null } | { kind: 'derivedFromInput' };
  connected: boolean;
}

function pinKindFromType(type: string): 'Data' | 'Exec' {
  return type === 'exec' ? 'Exec' : 'Data';
}

function formatDefinitionType(def: PinDefinitionDTO): string {
  if (def.kind === 'Exec') return 'exec';
  if (!def.dataType) return 'object';
  if (typeof def.dataType === 'object' && def.dataType !== null) {
    if ('Concrete' in def.dataType) {
      const dt = def.dataType.Concrete;
      if (typeof dt === 'object' && dt !== null && 'kind' in dt) {
        return String((dt as { kind: string }).kind);
      }
      return String(dt);
    }
    if ('TypeVar' in def.dataType) return def.dataType.TypeVar;
  }
  if (def.dataType === 'Unknown') return 'unknown';
  return 'unknown';
}

function slotNote(slot: PinSlot): ResolvedPinSpec['slotNote'] {
  if (slot.slotKind === 'repeatable') {
    return { kind: 'repeatableRange', min: slot.minCount, max: slot.maxCount };
  }
  if (slot.slotKind === 'derivedFromInput') {
    return { kind: 'derivedFromInput' };
  }
  return undefined;
}

function findDefinitionForPin(
  pin: PinView,
  slots: PinSlot[] | undefined,
): { optional: boolean; slotKind?: ResolvedPinSpec['slotKind']; slotNote?: string } {
  if (!slots?.length) return { optional: pin.optional ?? false };

  for (const slot of slots) {
    if (slot.slotKind === 'fixed') {
      const def = slot.pin;
      if (def.direction !== pin.direction) continue;
      if (def.name === pin.name) {
        return {
          optional: def.optional ?? pin.optional ?? false,
          slotKind: 'fixed',
        };
      }
    }
    if (slot.slotKind === 'repeatable') {
      const def = slot.template;
      if (def.direction !== pin.direction) continue;
      if (pin.name.startsWith(slot.namePrefix)) {
        return {
          optional: def.optional ?? pin.optional ?? false,
          slotKind: 'repeatable',
          slotNote: slotNote(slot),
        };
      }
    }
    if (slot.slotKind === 'derivedFromInput' && slot.direction === pin.direction) {
      return {
        optional: pin.optional ?? false,
        slotKind: 'derivedFromInput',
        slotNote: slotNote(slot),
      };
    }
  }

  return { optional: pin.optional ?? false };
}

function resolvePin(
  pin: PinView,
  definition: NodeDefinition | undefined,
): ResolvedPinSpec {
  const meta = findDefinitionForPin(pin, definition?.pinSlots);
  return {
    id: pin.id,
    name: pin.name,
    direction: pin.direction,
    kind: pinKindFromType(String(pin.type)),
    type: pin.typeDisplay ?? String(pin.type),
    typeDisplay: pin.typeDisplay,
    optional: meta.optional,
    slotKind: meta.slotKind,
    slotNote: meta.slotNote,
    connected: (pin.links?.length ?? 0) > 0,
  };
}

export function resolveNodePinSpecs(
  nodeId: string,
  pins: PinView[],
  definition: NodeDefinition | undefined,
): { inputs: ResolvedPinSpec[]; outputs: ResolvedPinSpec[] } {
  const nodePins = pins.filter((p) => p.nodeId === nodeId);
  const inputs = nodePins
    .filter((p) => p.direction === 'input')
    .map((p) => resolvePin(p, definition));
  const outputs = nodePins
    .filter((p) => p.direction === 'output')
    .map((p) => resolvePin(p, definition));
  return { inputs, outputs };
}

export function listDefinitionOnlyPins(
  definition: NodeDefinition | undefined,
): ResolvedPinSpec[] {
  if (!definition?.pinSlots) return [];

  const result: ResolvedPinSpec[] = [];
  for (const slot of definition.pinSlots) {
    if (slot.slotKind === 'fixed') {
      const def = slot.pin;
      result.push({
        id: `${def.direction}-${def.name}`,
        name: def.name,
        direction: def.direction,
        kind: def.kind,
        type: formatDefinitionType(def),
        optional: def.optional ?? false,
        slotKind: 'fixed',
        connected: false,
      });
    }
    if (slot.slotKind === 'repeatable') {
      const def = slot.template;
      result.push({
        id: `repeatable-${slot.namePrefix}`,
        name: `${slot.namePrefix}*`,
        direction: def.direction,
        kind: def.kind,
        type: formatDefinitionType(def),
        optional: def.optional ?? false,
        slotKind: 'repeatable',
        slotNote: slotNote(slot),
        connected: false,
      });
    }
  }
  return result;
}
