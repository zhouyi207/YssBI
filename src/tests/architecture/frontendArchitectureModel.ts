export {
  ModuleDependencyResolutionError,
  resolvedModuleDependencies,
  type ExternalDependencyOrigin,
  type FrontendDependencyMode,
  type ModuleDependencyKind,
  type ModuleDependencyMode,
  type ModuleDependencyOrigin,
  type ModuleDependencyResourceKind,
  type RepositoryAssetDependencyOrigin,
  type ResolvedModuleDependency,
  type StylesheetDependencyOrigin,
} from '@/tests/helpers/moduleDependencyAudit';
export {
  createRepositoryTextReader,
  resolvedStylesheetDependencies,
  type RepositoryTextReader,
  type ResolvedStylesheetDependency,
  type ResolvedStylesheetGraph,
  type StylesheetDependencyKind,
  type StylesheetResolutionError,
} from '@/tests/helpers/stylesheetDependencyAudit';
export { productionTypeScriptSources } from '@/tests/helpers/productionSourceAudit';
export {
  rawTauriInvokeOccurrences,
  type SourceOccurrence,
} from '@/tests/helpers/tauriInvokeAudit';
import type { ArchitectureSource } from '@/tests/helpers/moduleDependencyAudit';
import type {
  FrontendDependencyMode,
  ModuleDependencyKind,
  ModuleDependencyResourceKind,
  ResolvedModuleDependency,
  ModuleDependencyResolutionError,
} from '@/tests/helpers/moduleDependencyAudit';
import type {
  ResolvedStylesheetDependency,
  ResolvedStylesheetGraph,
  StylesheetDependencyKind,
  StylesheetResolutionError,
} from '@/tests/helpers/stylesheetDependencyAudit';

export type FrontendLayer =
  | 'app-composition'
  | 'views'
  | 'application'
  | 'core'
  | 'domain'
  | 'services'
  | 'components-ui'
  | 'wire-schema'
  | 'diagnostics'
  | 'pure-shared';

export type FrontendDependencyKind =
  | ModuleDependencyKind
  | StylesheetDependencyKind
  | 'call'
  | 'property-access'
  | 'constructor'
  | 'export-surface';

export type FrontendClassificationError =
  | {
      readonly kind: 'unclassified-production-source';
      readonly sourceFile: string;
    }
  | {
      readonly kind: 'multiply-classified-production-source';
      readonly sourceFile: string;
      readonly layers: readonly FrontendLayer[];
    };

export interface FrontendClassificationReport {
  readonly classification: ReadonlyMap<string, FrontendLayer>;
  readonly errors: readonly FrontendClassificationError[];
}

export type FrontendLiteralPolicyMembership = Readonly<
  Record<FrontendLayer, readonly string[]>
>;

export type FrontendBasePolicyMembership = FrontendLiteralPolicyMembership;

export interface FrontendResolvedCapability {
  readonly sourceLayer: 'app-composition' | 'views' | 'application';
  readonly canonicalModule: string;
  readonly exportedSymbols: readonly string[];
  readonly exactConsumers: readonly string[] | null;
  readonly memberCapabilities: Readonly<Record<string, readonly string[]>> | null;
}

export interface FrontendArchitecturePolicy {
  readonly layerEdges: readonly (readonly [FrontendLayer, FrontendLayer])[];
  readonly capabilities: readonly FrontendResolvedCapability[];
}

export interface FrontendFinding {
  readonly ruleId: string;
  readonly repositoryRelativeSourceFile: string;
  readonly fullyQualifiedOwner: string;
  readonly dependencyKind: FrontendDependencyKind;
  readonly canonicalOriginTarget: string;
  readonly sourceLayer: FrontendLayer;
  readonly targetLayer: FrontendLayer | null;
  readonly importedSymbol: string | null;
  readonly line: number;
  readonly column: number;
}

export interface ExternalDependencyAllowance {
  readonly sourceLayer: FrontendLayer;
  readonly mode: FrontendDependencyMode;
  readonly packageName: string;
  readonly resourceKind: ModuleDependencyResourceKind;
  readonly canonicalSubpaths: readonly (string | null)[];
  readonly consumerSourceFile: string | null;
}

export interface ExternalDependencyPolicy {
  readonly declaredRuntimePackages: readonly string[];
  readonly declaredBuildOnlyPackages: readonly string[];
  readonly uses: readonly ExternalDependencyAllowance[];
}

export interface RepositoryAssetDependencyAllowance {
  readonly sourceLayer: FrontendLayer;
  readonly mode: 'runtime' | 'build-style';
  readonly dependencyKind: 'side-effect-import' | StylesheetDependencyKind;
  readonly resourceKind: 'stylesheet';
  readonly consumerSourceFile: string;
  readonly repositoryRelativeAssetPath: string;
}

export interface AssetDependencyPolicy {
  readonly uses: readonly RepositoryAssetDependencyAllowance[];
}

export interface ReadonlyPackageManifest {
  readonly dependencies: Readonly<Record<string, string>>;
  readonly devDependencies: Readonly<Record<string, string>>;
}

export type FrontendExternalDependencyError =
  | {
      readonly kind: 'unknown-external-package';
      readonly packageName: string;
      readonly sourceFile: string;
    }
  | {
      readonly kind: 'development-dependency-in-production';
      readonly packageName: string;
      readonly sourceFile: string;
    }
  | {
      readonly kind: 'production-declaration-set-mismatch';
      readonly missing: readonly string[];
      readonly extra: readonly string[];
    }
  | {
      readonly kind: 'build-declaration-set-mismatch';
      readonly missing: readonly string[];
      readonly wrongScope: readonly string[];
    }
  | {
      readonly kind: 'invalid-external-policy-row';
      readonly packageName: string;
      readonly reason: string;
    };

export type FrontendAssetDependencyError =
  | StylesheetResolutionError
  | {
      readonly kind: 'stylesheet-layer-conflict';
      readonly sourceFile: string;
      readonly inheritedLayers: readonly FrontendLayer[];
    }
  | {
      readonly kind: 'invalid-asset-policy-row';
      readonly consumerSourceFile: string;
      readonly repositoryRelativeAssetPath: string;
      readonly reason: string;
    };

export interface FrontendExternalDependencyEvaluation {
  readonly sourceFile: string;
  readonly sourceLayer: FrontendLayer;
  readonly mode: FrontendDependencyMode;
  readonly packageName: string;
  readonly canonicalSubpath: string | null;
  readonly resourceKind: ModuleDependencyResourceKind;
  readonly consumerSourceFile: string | null;
  readonly declarationScope: 'production' | 'development';
  readonly allowed: boolean;
}

export interface FrontendExternalDependencyReport {
  readonly findings: readonly FrontendFinding[];
  readonly errors: readonly FrontendExternalDependencyError[];
  readonly evaluated: readonly FrontendExternalDependencyEvaluation[];
}

export interface FrontendAssetDependencyReport {
  readonly findings: readonly FrontendFinding[];
  readonly errors: readonly FrontendAssetDependencyError[];
  readonly stylesheetLayers: ReadonlyMap<string, FrontendLayer>;
}

export interface FrontendAssetAuditContext {
  readonly productionSources: readonly ArchitectureSource[];
  readonly moduleDependencies: readonly ResolvedModuleDependency[];
  readonly stylesheetGraph: ResolvedStylesheetGraph;
}

export interface FrontendDebtKey {
  readonly ruleId: string;
  readonly repositoryRelativeSourceFile: string;
  readonly fullyQualifiedOwner: string;
  readonly dependencyKind: FrontendDependencyKind;
  readonly canonicalOriginTarget: string;
}

export interface FrontendDebtEntry extends FrontendDebtKey {
  readonly expectedOccurrences: number;
  readonly owningMigrationSpec: string;
}

export type FrontendDebtDeclarationError =
  | {
      readonly kind: 'duplicate-frontend-debt-key';
      readonly key: FrontendDebtKey;
    }
  | {
      readonly kind: 'invalid-frontend-debt-count';
      readonly key: FrontendDebtKey;
      readonly expectedOccurrences: number;
    }
  | {
      readonly kind: 'invalid-frontend-debt-owning-spec';
      readonly key: FrontendDebtKey;
      readonly owningMigrationSpec: string;
    };

export interface FrontendDebtCountMismatch extends FrontendDebtKey {
  readonly actualOccurrences: number;
  readonly expectedOccurrences: number;
  readonly owningMigrationSpec: string | null;
}

export interface FrontendDebtMismatch {
  readonly newOrIncreased: readonly FrontendDebtCountMismatch[];
  readonly staleOrDecreased: readonly FrontendDebtCountMismatch[];
  readonly errors: readonly FrontendDebtDeclarationError[];
}

export interface FrontendArchitectureReport {
  readonly classification: FrontendClassificationReport;
  readonly unresolvedErrors: readonly ModuleDependencyResolutionError[];
  readonly moduleDependencies: readonly ResolvedModuleDependency[];
  readonly stylesheetGraph: ResolvedStylesheetGraph;
  readonly external: FrontendExternalDependencyReport;
  readonly asset: FrontendAssetDependencyReport;
  readonly findings: readonly FrontendFinding[];
  readonly debt: FrontendDebtMismatch;
}

export type ResolvedFrontendDependency =
  | ResolvedModuleDependency
  | ResolvedStylesheetDependency;

export type { ArchitectureSource };
