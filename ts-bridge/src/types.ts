export type JsonRpcId = number | string | null;

export const ExhaustivenessSiteKind = {
  Switch: "switch",
  Record: "record",
  Satisfies: "satisfies",
  IfElse: "if_else",
} as const;

export type ExhaustivenessSiteKind =
  (typeof ExhaustivenessSiteKind)[keyof typeof ExhaustivenessSiteKind];

export const ExhaustivenessProofKind = {
  Switch: "switch",
  AssertNever: "assertNever",
  Record: "Record",
  Satisfies: "satisfies",
  IfElse: "if_else",
} as const;

export type ExhaustivenessProofKind =
  (typeof ExhaustivenessProofKind)[keyof typeof ExhaustivenessProofKind];

export const ExhaustivenessFallbackKind = {
  None: "none",
  Null: "null",
  Undefined: "undefined",
  GenericString: "generic_string",
  IdentityTransform: "identity_transform",
  EmptyArray: "empty_array",
  EmptyObject: "empty_object",
  AssertThrow: "assert_throw",
  Other: "other",
} as const;

export type ExhaustivenessFallbackKind =
  (typeof ExhaustivenessFallbackKind)[keyof typeof ExhaustivenessFallbackKind];

export const ExhaustivenessSiteSemanticRole = {
  Label: "label",
  Target: "target",
  Status: "status",
  Render: "render",
  Handler: "handler",
  Policy: "policy",
  Serialization: "serialization",
  Transform: "transform",
  Unknown: "unknown",
} as const;

export type ExhaustivenessSiteSemanticRole =
  (typeof ExhaustivenessSiteSemanticRole)[keyof typeof ExhaustivenessSiteSemanticRole];

export const TransitionKind = {
  RecordEntry: "record_entry",
  SwitchCase: "switch_case",
  IfBranch: "if_branch",
  IfElse: "if_else",
} as const;

export type TransitionKind = (typeof TransitionKind)[keyof typeof TransitionKind];

export interface JsonRpcRequest {
  jsonrpc?: string;
  id?: JsonRpcId;
  method?: string;
  params?: unknown;
}

export interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: JsonRpcId;
  result?: unknown;
  error?: JsonRpcError;
}

export interface JsonRpcError {
  code: number;
  message: string;
  data?: unknown;
}

export interface ProjectModel {
  root: string;
  tsconfig_paths: string[];
  workspace_files: string[];
  primary_language?: string | null;
  fingerprint: string;
  repo_archetype?: string | null;
  detected_archetypes: ProjectArchetypeMatch[];
}

export interface ProjectArchetypeMatch {
  id: string;
  confidence: string;
  reasons: string[];
}

export interface SemanticSnapshot {
  project: ProjectModel;
  analyzed_files: number;
  capabilities: string[];
  files: SemanticFileFact[];
  symbols: SymbolFact[];
  reads: ReadFact[];
  writes: WriteFact[];
  closed_domains: ClosedDomain[];
  closed_domain_sites: ExhaustivenessSite[];
  transition_sites: TransitionSite[];
}

export type AnalyzeProjectsResult = SemanticSnapshot;

export interface SemanticFileFact {
  path: string;
  symbol_count: number;
  write_count: number;
  closed_domain_count: number;
}

export interface SymbolFact {
  id: string;
  path: string;
  name: string;
  kind: string;
  line: number;
}

export interface ReadFact {
  path: string;
  symbol_name: string;
  read_kind: string;
  line: number;
}

export interface WriteFact {
  path: string;
  symbol_name: string;
  write_kind: string;
  line: number;
}

export interface ClosedDomain {
  path: string;
  symbol_name: string;
  variants: string[];
  line: number;
  defining_file: string | null;
}

export interface ExhaustivenessSite {
  path: string;
  domain_symbol_name: string;
  defining_file: string | null;
  site_kind: ExhaustivenessSiteKind;
  proof_kind: ExhaustivenessProofKind;
  covered_variants: string[];
  line: number;
  fallback_kind?: ExhaustivenessFallbackKind | null;
  site_expression?: string | null;
  site_semantic_role?: ExhaustivenessSiteSemanticRole | null;
  site_confidence?: number | null;
}

export interface TransitionSite {
  path: string;
  domain_symbol_name: string;
  group_id: string;
  transition_kind: TransitionKind;
  source_variant?: string | null;
  target_variants: string[];
  line: number;
}
