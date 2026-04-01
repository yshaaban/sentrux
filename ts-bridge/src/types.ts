export type JsonRpcId = number | string | null;

export const ExhaustivenessSiteKind = {
  Switch: "switch",
  Record: "record",
  Satisfies: "satisfies",
} as const;

export type ExhaustivenessSiteKind =
  (typeof ExhaustivenessSiteKind)[keyof typeof ExhaustivenessSiteKind];

export const ExhaustivenessProofKind = {
  Switch: "switch",
  AssertNever: "assertNever",
  Record: "Record",
  Satisfies: "satisfies",
} as const;

export type ExhaustivenessProofKind =
  (typeof ExhaustivenessProofKind)[keyof typeof ExhaustivenessProofKind];

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
}

export interface ExhaustivenessSite {
  path: string;
  domain_symbol_name: string;
  site_kind: ExhaustivenessSiteKind;
  proof_kind: ExhaustivenessProofKind;
  covered_variants: string[];
  line: number;
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
