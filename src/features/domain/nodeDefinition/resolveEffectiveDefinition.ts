import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import type { FunctionSignaturePin } from '@/shared/types/domain/graph';
import { dataTypeFromFunctionSignaturePin } from '@/shared/types/domain/dataType';
import type {
  NodeDefinition,
  PinDefinitionDTO,
  PinDirection,
  PinSlot,
  PinTypeCapability,
} from '@/shared/types/domain/node';

export const CALL_FUNCTION_NODE_TYPE = 'Functions:Call Function';

const DYNAMIC_PIN_META = {
  showWidget: false,
  widgetType: null,
  isDynamic: true,
} as const;

/** 对齐 Rust `default_function_exec_input` / `default_function_exec_output`。 */
export const DEFAULT_FUNCTION_EXEC_INPUT: FunctionSignaturePin = {
  id: 'exec-in',
  name: 'In',
  type: 'exec',
};

export const DEFAULT_FUNCTION_EXEC_OUTPUT: FunctionSignaturePin = {
  id: 'exec-out',
  name: 'Out',
  type: 'exec',
};

export function defaultFunctionSignature(): {
  functionInputs: FunctionSignaturePin[];
  functionOutputs: FunctionSignaturePin[];
} {
  return {
    functionInputs: [DEFAULT_FUNCTION_EXEC_INPUT],
    functionOutputs: [DEFAULT_FUNCTION_EXEC_OUTPUT],
  };
}

export type ResolveEffectiveOptions = {
  subGraphId?: string;
  functionInputs?: FunctionSignaturePin[];
  functionOutputs?: FunctionSignaturePin[];
};

function signaturePinToDefinition(
  sig: FunctionSignaturePin,
  direction: PinDirection,
): PinDefinitionDTO {
  if (sig.type === 'exec') {
    return {
      name: sig.name,
      direction,
      kind: 'Exec',
      role: { Exec: { Custom: sig.id } },
      dataType: null,
      optional: false,
      metaData: { ...DYNAMIC_PIN_META },
    };
  }

  const dataType = dataTypeFromFunctionSignaturePin(sig);
  return {
    name: sig.name,
    direction,
    kind: 'Data',
    role: { Data: { Custom: sig.id } },
    dataType: { Concrete: dataType },
    optional: false,
    metaData: { ...DYNAMIC_PIN_META },
  };
}

/**
 * 函数签名 → 固定 pinSlots（顺序对齐 Rust `sync_call_function_pins_from_signature`：
 * inputs → outputs）。
 */
export function signatureToPinSlots(
  functionInputs: FunctionSignaturePin[],
  functionOutputs: FunctionSignaturePin[],
): PinSlot[] {
  const slots: PinSlot[] = [];
  for (const sig of functionInputs) {
    slots.push({
      slotKind: 'fixed',
      pin: signaturePinToDefinition(sig, 'input'),
    });
  }
  for (const sig of functionOutputs) {
    slots.push({
      slotKind: 'fixed',
      pin: signaturePinToDefinition(sig, 'output'),
    });
  }
  return slots;
}

/** 从 pinSlots 推导 typeCapabilities（供拖 pin 过滤 / 自动连线）。 */
export function typeCapabilitiesFromPinSlots(slots: PinSlot[]): PinTypeCapability[] {
  const caps: PinTypeCapability[] = [];
  for (const slot of slots) {
    if (slot.slotKind !== 'fixed') continue;
    const pin = slot.pin;
    if (pin.kind === 'Exec') {
      caps.push({ direction: pin.direction, kind: 'Exec', dataType: 'Unknown' });
      continue;
    }
    if (pin.dataType) {
      caps.push({ direction: pin.direction, kind: 'Data', dataType: pin.dataType });
    }
  }
  return caps;
}

function resolveSignature(options: ResolveEffectiveOptions): {
  functionInputs: FunctionSignaturePin[];
  functionOutputs: FunctionSignaturePin[];
} {
  if (options.functionInputs !== undefined || options.functionOutputs !== undefined) {
    const defaults = defaultFunctionSignature();
    return {
      functionInputs: options.functionInputs ?? defaults.functionInputs,
      functionOutputs: options.functionOutputs ?? defaults.functionOutputs,
    };
  }

  const defaults = defaultFunctionSignature();
  if (!options.subGraphId) return defaults;

  const meta = useGraphMetaStore.getState().graphs[options.subGraphId];
  return {
    functionInputs: meta?.functionInputs ?? defaults.functionInputs,
    functionOutputs: meta?.functionOutputs ?? defaults.functionOutputs,
  };
}

/**
 * 将注册表定义解析为实例有效定义。
 * Call Function + subGraphId/签名 → 注入投影后的 pinSlots / typeCapabilities。
 */
export function resolveEffectiveDefinition(
  base: NodeDefinition,
  options?: ResolveEffectiveOptions,
): NodeDefinition {
  if (base.nodeType !== CALL_FUNCTION_NODE_TYPE || !options?.subGraphId) {
    return base;
  }

  const { functionInputs, functionOutputs } = resolveSignature(options);
  const pinSlots = signatureToPinSlots(functionInputs, functionOutputs);

  return {
    ...base,
    pinSlots,
    typeCapabilities: typeCapabilitiesFromPinSlots(pinSlots),
  };
}
