import type ts from "typescript";

import type {
  ClosedDomain,
  ExhaustivenessSite,
  ReadFact,
  SymbolFact,
  TransitionSite,
  WriteFact,
} from "./types.js";

export interface FileAnalysisContext {
  rootPath: string;
  relativePath: string;
  sourceFile: ts.SourceFile;
  checker: ts.TypeChecker;
  symbolFacts: SymbolFact[];
  readFacts: ReadFact[];
  writeFacts: WriteFact[];
  closedDomains: ClosedDomain[];
  closedDomainSites: ExhaustivenessSite[];
  transitionSites: TransitionSite[];
}

export interface ClosedDomainInfo {
  domainSymbolName: string;
  definingFile: string | null;
  variants: string[];
}
