export type ResourceBoundCreateArgsDto =
  | { kind: "function" }
  | { kind: "variable" }
  | { kind: "database" };

export type NodeCreationDescriptorDto =
  | {
      kind: "static";
      nodeTypeId: string;
    }
  | {
      kind: "parameterizedStatic";
      nodeTypeId: string;
      requiredParameters: string[];
    }
  | {
      kind: "resourceBound";
      nodeTypeId: string;
      resourcePath: string;
      resourceRevision: number;
      createArgs: ResourceBoundCreateArgsDto;
    };

function isExactRecord(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const actualKeys = Object.keys(value);
  return actualKeys.length === keys.length && keys.every((key) => actualKeys.includes(key));
}

function isResourceBoundCreateArgs(value: unknown): value is ResourceBoundCreateArgsDto {
  return (
    isExactRecord(value, ["kind"]) &&
    (value.kind === "function" || value.kind === "variable" || value.kind === "database")
  );
}

export function isNodeCreationDescriptorDto(value: unknown): value is NodeCreationDescriptorDto {
  if (isExactRecord(value, ["kind", "nodeTypeId"])) {
    return value.kind === "static" && typeof value.nodeTypeId === "string";
  }
  if (isExactRecord(value, ["kind", "nodeTypeId", "requiredParameters"])) {
    return (
      value.kind === "parameterizedStatic" &&
      typeof value.nodeTypeId === "string" &&
      Array.isArray(value.requiredParameters) &&
      value.requiredParameters.every((key) => typeof key === "string")
    );
  }
  if (
    !isExactRecord(value, ["kind", "nodeTypeId", "resourcePath", "resourceRevision", "createArgs"])
  )
    return false;
  return (
    value.kind === "resourceBound" &&
    typeof value.nodeTypeId === "string" &&
    typeof value.resourcePath === "string" &&
    Number.isSafeInteger(value.resourceRevision) &&
    (value.resourceRevision as number) >= 0 &&
    isResourceBoundCreateArgs(value.createArgs)
  );
}
