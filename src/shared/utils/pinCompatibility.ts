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
  Pick<PinData, 'connections' | 'kind' | 'orphan' | 'resolvedType'>
>;

export type TypeCompatibility = 'compatible' | 'incompatible' | 'indeterminate';

function everyCompatibility(results: TypeCompatibility[]): TypeCompatibility {
  if (results.some((result) => result === 'incompatible')) return 'incompatible';
  if (results.some((result) => result === 'indeterminate')) return 'indeterminate';
  return 'compatible';
}

function someCompatibility(results: TypeCompatibility[]): TypeCompatibility {
  if (results.some((result) => result === 'compatible')) return 'compatible';
  if (results.some((result) => result === 'indeterminate')) return 'indeterminate';
  return 'incompatible';
}

export function getDataTypeCompatibility(
  source: DataType | null | undefined,
  target: DataType | null | undefined,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): TypeCompatibility {
  if (!source || !target) return 'indeterminate';
  if (source.kind === 'OneOf') {
    return everyCompatibility(source.inner.map((member) =>
      getDataTypeCompatibility(member, target, typeSystem)));
  }
  if (target.kind === 'OneOf') {
    return someCompatibility(target.inner.map((member) =>
      getDataTypeCompatibility(source, member, typeSystem)));
  }
  if (target.kind !== source.kind) return 'incompatible';
  if (target.kind === 'Array' && source.kind === 'Array') {
    return getDataTypeCompatibility(source.inner, target.inner, typeSystem);
  }
  if (target.kind === 'DataSeries' && source.kind === 'DataSeries') {
    return getDataTypeCompatibility(source.inner, target.inner, typeSystem);
  }
  if (target.kind === 'Struct' && source.kind === 'Struct') {
    return structCanAccept(target.inner, source.inner, typeSystem)
      ? 'compatible'
      : 'incompatible';
  }
  return 'compatible';
}

function canAcceptDataType(
  target: DataType,
  source: DataType,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): boolean {
  return getDataTypeCompatibility(source, target, typeSystem) === 'compatible';
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
  const source = candidate.direction === 'output' ? candidate : dragged;
  const target = candidate.direction === 'input' ? candidate : dragged;
  return getPinCompatibility(source, target, typeSystem) === 'compatible';
}

function projectedPinDataType(pin: ConnectionCandidatePin): DataType | null | undefined {
  if (pin.resolvedType) {
    return pin.resolvedType.resolved ? pin.resolvedType.dataType : null;
  }
  return pin.dataType;
}

export function getPinCompatibility(
  source: ConnectionCandidatePin,
  target: ConnectionCandidatePin,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): TypeCompatibility {
  if (source.id === target.id
    || source.nodeId === target.nodeId
    || source.direction !== 'output'
    || target.direction !== 'input') return 'incompatible';

  const sourceIsExec = isExecPin(source);
  const targetIsExec = isExecPin(target);
  if (sourceIsExec !== targetIsExec) return 'incompatible';
  if (sourceIsExec) return 'compatible';
  return getDataTypeCompatibility(
    projectedPinDataType(source),
    projectedPinDataType(target),
    typeSystem,
  );
}

export type ConnectionInvalidReason =
  | 'samePort'
  | 'sameNode'
  | 'directionMismatch'
  | 'kindMismatch'
  | 'typeMismatch'
  | 'orphan'
  | 'capacityReached';

export type ConnectionCompatibility =
  | { kind: 'append' }
  | { kind: 'replace' }
  | { kind: 'invalid'; reason: ConnectionInvalidReason };

function connectionKind(pin: ConnectionCandidatePin): 'data' | 'control' | 'effect' {
  return pin.kind ?? (isExecPin(pin) ? 'control' : 'data');
}

function canAppendOrReplace(pin: ConnectionCandidatePin): boolean {
  return pin.connections
    ? pin.connections.canAppend || pin.connections.canReplace
    : true;
}

export function resolveConnectionCompatibility(
  a: ConnectionCandidatePin,
  b: ConnectionCandidatePin,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): ConnectionCompatibility {
  if (a.id === b.id) return { kind: 'invalid', reason: 'samePort' };
  if (a.nodeId === b.nodeId) return { kind: 'invalid', reason: 'sameNode' };
  if (a.direction === b.direction) return { kind: 'invalid', reason: 'directionMismatch' };

  const source = a.direction === 'output' ? a : b;
  const target = a.direction === 'input' ? a : b;
  const sourceKind = connectionKind(source);
  const targetKind = connectionKind(target);

  if (sourceKind !== targetKind) return { kind: 'invalid', reason: 'kindMismatch' };
  if (source.orphan || target.orphan) return { kind: 'invalid', reason: 'orphan' };
  if (!canAppendOrReplace(source) || !canAppendOrReplace(target)) {
    return { kind: 'invalid', reason: 'capacityReached' };
  }

  if (sourceKind === 'data'
    && getPinCompatibility(source, target, typeSystem) === 'incompatible') {
    return { kind: 'invalid', reason: 'typeMismatch' };
  }

  return source.connections?.canReplace || target.connections?.canReplace
    ? { kind: 'replace' }
    : { kind: 'append' };
}

export function canConnectPins(
  a: ConnectionCandidatePin,
  b: ConnectionCandidatePin,
  typeSystem: TypeSystemSnapshot = EMPTY_TYPE_SYSTEM,
): boolean {
  return resolveConnectionCompatibility(a, b, typeSystem).kind !== 'invalid';
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
