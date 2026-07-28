import type { DataType } from '@/shared/types/domain/dataType';
import type { Pin, PinDirection } from '@/shared/types/domain/pin';
import { isExecPin } from '@/shared/types/domain/pinSemantics';
import { EMPTY_TYPE_SYSTEM, type TypeSystemSnapshot } from '@/shared/types/domain/typeSystem';
import type {
  PinDataTypeDefinition,
  PinTypeCapability,
  NodeDefinition,
  PinSlot,
  PinDefinitionDTO,
} from '@/shared/types/domain/node';
import { structCanAccept } from '@/shared/types/domain/typeSystem';
import type { PinData } from '@/shared/types/store/graph';

export type ConnectionCandidatePin = Pin & Partial<
  Pick<PinData, 'connections' | 'kind' | 'resolvedType'>
>;

/**
 * Mirror of backend DataType.can_accept():
 * exact match, Any wildcard, recursive container check.
 */
function canAcceptDataType(
  target: DataType,
  source: DataType,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): boolean {
  if (target.kind === source.kind) {
    if (target.kind === 'Array' && source.kind === 'Array') {
      return canAcceptDataType(target.inner, source.inner, typeSystem);
    }
    if (target.kind === 'DataSeries' && source.kind === 'DataSeries') {
      return canAcceptDataType(target.inner, source.inner, typeSystem);
    }
    if (target.kind === 'Struct' && source.kind === 'Struct') {
      return structCanAccept(target.inner, source.inner, typeSystem);
    }
    return true;
  }
  if (target.kind === 'Any' || source.kind === 'Any') return true;
  if (target.kind === 'OneOf') {
    return target.inner.some(t => canAcceptDataType(t, source, typeSystem));
  }
  if (source.kind === 'OneOf') {
    return source.inner.some(s => canAcceptDataType(target, s, typeSystem));
  }
  return false;
}

/**
 * 取得 pin 的结构化 DataType（类型判断的唯一来源）。
 * Exec pin 不进入数据类型系统；Data pin 缺少 `dataType` 是 schema/乐观创建错误。
 */
export function buildPinDataType(pin: Pin): DataType {
  if (isExecPin(pin)) return { kind: 'Any' };
  if (pin.dataType) return pin.dataType;
  throw new Error(`Pin ${pin.id} (${pin.name}) is missing structured dataType`);
}

/**
 * 被拖拽 pin 与一个候选类型是否可连接（方向感知）。
 * - dragged 为 input:候选方为 output 产出 candidateType,需 dragged 的输入能接受它
 * - dragged 为 output:候选方为 input,需其能接受 dragged 的输出类型
 */
export function pinAcceptsType(
  draggedPin: Pin,
  candidateType: DataType,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): boolean {
  const draggedType = buildPinDataType(draggedPin);
  if (draggedPin.direction === 'input') {
    return canAcceptDataType(draggedType, candidateType, typeSystem);
  }
  return canAcceptDataType(candidateType, draggedType, typeSystem);
}

export function isPinCompatible(
  candidate: ConnectionCandidatePin,
  dragged: ConnectionCandidatePin,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): boolean {
  return canConnectPins(candidate, dragged, typeSystem);
}

export function canConnectPins(
  a: ConnectionCandidatePin,
  b: ConnectionCandidatePin,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): boolean {
  if (a.id === b.id) return false;
  if (a.nodeId === b.nodeId) return false;
  if (a.direction === b.direction) return false;

  const source = a.direction === 'output' ? a : b;
  const target = a.direction === 'input' ? a : b;

  const sourceIsExec = isExecPin(source);
  const targetIsExec = isExecPin(target);
  if (sourceIsExec !== targetIsExec) return false;
  if (sourceIsExec) return true;

  if (source.connections?.canConnect === false || target.connections?.canConnect === false) {
    return false;
  }

  if (!source.dataType || !target.dataType) {
    return source.kind === 'data' && target.kind === 'data';
  }

  return canAcceptDataType(target.dataType, source.dataType, typeSystem);
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
function isCapabilityCompatible(
  cap: PinTypeCapability,
  draggedPin: Pin,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): boolean {
  const neededDir: PinDirection = draggedPin.direction === 'input' ? 'output' : 'input';
  if (cap.direction !== neededDir) return false;

  const draggedIsExec = isExecPin(draggedPin);
  if (draggedIsExec) return cap.kind === 'Exec';
  if (cap.kind === 'Exec') return false;

  const concreteType = extractConcreteType(cap.dataType);
  if (!concreteType) return true; // TypeVar / Unknown -> wildcard

  const draggedDataType = buildPinDataType(draggedPin);
  if (draggedPin.direction === 'input') {
    return canAcceptDataType(draggedDataType, concreteType, typeSystem);
  }
  return canAcceptDataType(concreteType, draggedDataType, typeSystem);
}

/**
 * Check if a NodeDefinition has at least one pin capability
 * compatible with the dragged pin.
 */
export function isNodeCompatibleWithPin(
  def: NodeDefinition,
  draggedPin: Pin,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): boolean {
  if (!def.typeCapabilities || def.typeCapabilities.length === 0) return true;
  return def.typeCapabilities.some((cap) => isCapabilityCompatible(cap, draggedPin, typeSystem));
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
function generateInitialPinsFromSlots(slots: PinSlot[]): PinDefinitionDTO[] {
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
export function findAutoConnectPinIndex(
  slots: PinSlot[],
  draggedPin: Pin,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): number {
  const targetDir: PinDirection = draggedPin.direction === 'input' ? 'output' : 'input';
  const draggedIsExec = isExecPin(draggedPin);
  const draggedDataType = draggedIsExec ? null : buildPinDataType(draggedPin);

  const initialPins = generateInitialPinsFromSlots(slots);
  for (let i = 0; i < initialPins.length; i++) {
    const p = initialPins[i];
    if (p.direction !== targetDir) continue;

    if (draggedIsExec) {
      if (p.kind === 'Exec') return i;
      continue;
    }
    if (p.kind === 'Exec') continue;

    if (!p.dataType) return i;
    const concrete = extractConcreteType(p.dataType);
    if (!concrete) return i; // TypeVar/Unknown -> compatible
    if (draggedPin.direction === 'input') {
      if (canAcceptDataType(draggedDataType!, concrete, typeSystem)) return i;
    } else {
      if (canAcceptDataType(concrete, draggedDataType!, typeSystem)) return i;
    }
  }
  return -1;
}
