import type { DataType } from '@/shared/types/domain/dataType';
import type { Pin, PinDirection } from '@/shared/types/domain/pin';
import type {
  PinDataTypeDefinition,
  PinTypeCapability,
  NodeDefinition,
  PinSlot,
  PinDefinitionDTO,
} from '@/shared/types/domain/node';
import { dataTypeFromPinType, dataTypeFromDisplayString } from '@/shared/types/domain/dataType';

/**
 * Mirror of backend DataType.can_accept():
 * exact match, Any wildcard, recursive container check.
 */
function canAcceptDataType(target: DataType, source: DataType): boolean {
  if (target.kind === source.kind) {
    if (target.kind === 'Array' && source.kind === 'Array') {
      return canAcceptDataType(target.inner, source.inner);
    }
    if (target.kind === 'DataSeries' && source.kind === 'DataSeries') {
      return canAcceptDataType(target.inner, source.inner);
    }
    if (target.kind === 'Struct' && source.kind === 'Struct') {
      return target.inner === source.inner;
    }
    return true;
  }
  if (target.kind === 'Any' || source.kind === 'Any') return true;
  if (target.kind === 'OneOf') {
    return target.inner.some(t => canAcceptDataType(t, source));
  }
  if (source.kind === 'OneOf') {
    return source.inner.some(s => canAcceptDataType(target, s));
  }
  return false;
}

/**
 * 取得 pin 的结构化 DataType（兼容判断的唯一来源）。
 * 优先用后端下发的结构化 `dataType`;无则回退解析 `typeDisplay`/`type`
 * （兼容尚未携带 dataType 的乐观 pin 或旧数据）。
 */
export function buildPinDataType(pin: Pin): DataType {
  if (pin.dataType) return pin.dataType;
  if (pin.typeDisplay) {
    const parsed = dataTypeFromDisplayString(pin.typeDisplay);
    if (parsed) return parsed;
  }
  const base = dataTypeFromPinType(pin.type);
  if (pin.containerType === 'array') return { kind: 'Array', inner: base };
  if (pin.containerType === 'dataseries') return { kind: 'DataSeries', inner: base };
  return base;
}

/**
 * 被拖拽 pin 与一个候选类型是否可连接（方向感知）。
 * - dragged 为 input:候选方为 output 产出 candidateType,需 dragged 的输入能接受它
 * - dragged 为 output:候选方为 input,需其能接受 dragged 的输出类型
 */
export function pinAcceptsType(draggedPin: Pin, candidateType: DataType): boolean {
  const draggedType = buildPinDataType(draggedPin);
  if (draggedPin.direction === 'input') {
    return canAcceptDataType(draggedType, candidateType);
  }
  return canAcceptDataType(candidateType, draggedType);
}

export function isPinCompatible(candidate: Pin, dragged: Pin): boolean {
  if (candidate.direction === dragged.direction) return false;
  if (candidate.nodeId === dragged.nodeId) return false;

  const candidateIsExec = candidate.type === 'exec';
  const draggedIsExec = dragged.type === 'exec';
  if (candidateIsExec !== draggedIsExec) return false;
  if (candidateIsExec) return true;

  return pinAcceptsType(dragged, buildPinDataType(candidate));
}

function extractConcreteType(pdt: PinDataTypeDefinition): DataType | null {
  if (pdt === 'Unknown') return null;
  if ('Concrete' in pdt) return pdt.Concrete;
  if ('TypeVar' in pdt) return null;
  return null;
}

/**
 * Check if a PinTypeCapability is compatible with a dragged Pin.
 * TypeVar / Unknown capabilities are treated as wildcard (always compatible).
 */
function isCapabilityCompatible(cap: PinTypeCapability, draggedPin: Pin): boolean {
  const neededDir: PinDirection = draggedPin.direction === 'input' ? 'output' : 'input';
  if (cap.direction !== neededDir) return false;

  const isExecPin = draggedPin.type === 'exec';
  if (isExecPin) return cap.kind === 'Exec';
  if (cap.kind === 'Exec') return false;

  const concreteType = extractConcreteType(cap.dataType);
  if (!concreteType) return true; // TypeVar / Unknown -> wildcard

  const draggedDataType = buildPinDataType(draggedPin);
  if (draggedPin.direction === 'input') {
    return canAcceptDataType(draggedDataType, concreteType);
  }
  return canAcceptDataType(concreteType, draggedDataType);
}

/**
 * Check if a NodeDefinition has at least one pin capability
 * compatible with the dragged pin.
 */
export function isNodeCompatibleWithPin(def: NodeDefinition, draggedPin: Pin): boolean {
  if (!def.typeCapabilities || def.typeCapabilities.length === 0) return true;
  return def.typeCapabilities.some((cap) => isCapabilityCompatible(cap, draggedPin));
}

// ─── Utilities for auto-connect pin matching ────

function generateSlotName(prefix: string, index: number): string {
  if (!prefix) {
    return index < 26 ? String.fromCharCode(65 + index) : `Pin ${index}`;
  }
  return `${prefix} ${index + 1}`;
}

/**
 * Generate the ordered list of initial PinDefinitionDTOs from pin slots.
 * Matches the backend's generate_initial_pins() ordering.
 */
export function generateInitialPinsFromSlots(slots: PinSlot[]): PinDefinitionDTO[] {
  const pins: PinDefinitionDTO[] = [];
  for (const slot of slots) {
    if (slot.slotKind === 'fixed') {
      pins.push(slot.pin);
    } else if (slot.slotKind === 'repeatable') {
      for (let i = 0; i < slot.minCount; i++) {
        pins.push({ ...slot.template, name: generateSlotName(slot.namePrefix, i) });
      }
    }
    // derivedFromInput produces no initial pins
  }
  return pins;
}

/**
 * Find the index (within pinIds) of the first pin that should be
 * auto-connected to the dragged pin.
 */
export function findAutoConnectPinIndex(slots: PinSlot[], draggedPin: Pin): number {
  const targetDir: PinDirection = draggedPin.direction === 'input' ? 'output' : 'input';
  const isExecPin = draggedPin.type === 'exec';
  const draggedDataType = isExecPin ? null : buildPinDataType(draggedPin);

  const initialPins = generateInitialPinsFromSlots(slots);
  for (let i = 0; i < initialPins.length; i++) {
    const p = initialPins[i];
    if (p.direction !== targetDir) continue;

    if (isExecPin) {
      if (p.kind === 'Exec') return i;
      continue;
    }
    if (p.kind === 'Exec') continue;

    if (!p.dataType) return i;
    const concrete = extractConcreteType(p.dataType);
    if (!concrete) return i; // TypeVar/Unknown -> compatible
    if (draggedPin.direction === 'input') {
      if (canAcceptDataType(draggedDataType!, concrete)) return i;
    } else {
      if (canAcceptDataType(concrete, draggedDataType!)) return i;
    }
  }
  return -1;
}
