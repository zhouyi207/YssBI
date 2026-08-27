export interface ResourceReadSnapshot {
  readonly resourcePath: string;
  readonly revision: number;
  readonly loaded: boolean;
  readonly dirty: boolean;
}

export interface ResourceReadCapability {
  readonly getResource: (resourcePath: string) => ResourceReadSnapshot | null;
  readonly listResources: () => readonly ResourceReadSnapshot[];
}
