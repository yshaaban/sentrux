import fs from "node:fs";
import path from "node:path";
import ts from "typescript";

type JsonRpcId = number | string | null;

const PROTOCOL_VERSION = "0.1.0";

interface JsonRpcRequest {
  jsonrpc?: string;
  id?: JsonRpcId;
  method?: string;
  params?: unknown;
}

interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: JsonRpcId;
  result?: unknown;
  error?: JsonRpcError;
}

interface JsonRpcError {
  code: number;
  message: string;
  data?: unknown;
}

interface ProjectModel {
  root: string;
  tsconfig_paths: string[];
  workspace_files: string[];
  primary_language?: string | null;
  fingerprint: string;
  repo_archetype?: string | null;
  detected_archetypes: ProjectArchetypeMatch[];
}

interface ProjectArchetypeMatch {
  id: string;
  confidence: string;
  reasons: string[];
}

interface SemanticSnapshot {
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

interface SemanticFileFact {
  path: string;
  symbol_count: number;
  write_count: number;
  closed_domain_count: number;
}

interface SymbolFact {
  id: string;
  path: string;
  name: string;
  kind: string;
  line: number;
}

interface ReadFact {
  path: string;
  symbol_name: string;
  read_kind: string;
  line: number;
}

interface WriteFact {
  path: string;
  symbol_name: string;
  write_kind: string;
  line: number;
}

interface ClosedDomain {
  path: string;
  symbol_name: string;
  variants: string[];
  line: number;
}

interface ExhaustivenessSite {
  path: string;
  domain_symbol_name: string;
  site_kind: string;
  proof_kind: string;
  covered_variants: string[];
  line: number;
}

interface TransitionSite {
  path: string;
  domain_symbol_name: string;
  group_id: string;
  transition_kind: string;
  source_variant?: string | null;
  target_variants: string[];
  line: number;
}

interface ClosedDomainInfo {
  domainSymbolName: string;
  variants: string[];
}

interface FileAnalysisContext {
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

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isStringArray(value: unknown): value is string[] {
  if (!Array.isArray(value)) {
    return false;
  }

  for (const entry of value) {
    if (typeof entry !== "string") {
      return false;
    }
  }

  return true;
}

function toRequest(value: unknown): JsonRpcRequest | null {
  if (!isObject(value)) {
    return null;
  }

  return {
    jsonrpc: typeof value.jsonrpc === "string" ? value.jsonrpc : undefined,
    id: value.id as JsonRpcId,
    method: typeof value.method === "string" ? value.method : undefined,
    params: value.params,
  };
}

function toProjectModel(value: unknown): ProjectModel | null {
  if (!isObject(value)) {
    return null;
  }

  if (typeof value.root !== "string" || typeof value.fingerprint !== "string") {
    return null;
  }
  if (!isStringArray(value.tsconfig_paths) || !isStringArray(value.workspace_files)) {
    return null;
  }

  return {
    root: value.root,
    tsconfig_paths: value.tsconfig_paths,
    workspace_files: value.workspace_files,
    primary_language:
      typeof value.primary_language === "string" ? value.primary_language : null,
    fingerprint: value.fingerprint,
    repo_archetype:
      typeof value.repo_archetype === "string" ? value.repo_archetype : null,
    detected_archetypes: Array.isArray(value.detected_archetypes)
      ? value.detected_archetypes.flatMap((entry) => {
          if (!isObject(entry)) {
            return [];
          }
          if (
            typeof entry.id !== "string" ||
            typeof entry.confidence !== "string" ||
            !isStringArray(entry.reasons)
          ) {
            return [];
          }
          return [
            {
              id: entry.id,
              confidence: entry.confidence,
              reasons: entry.reasons,
            },
          ];
        })
      : [],
  };
}

function normalizePath(value: string): string {
  return value.replaceAll("\\", "/");
}

function relativePath(rootPath: string, filePath: string): string {
  const relative = path.relative(rootPath, filePath);
  return normalizePath(relative.length > 0 ? relative : path.basename(filePath));
}

function lineOfNode(sourceFile: ts.SourceFile, node: ts.Node): number {
  return ts.getLineAndCharacterOfPosition(sourceFile, node.getStart(sourceFile)).line + 1;
}

function symbolId(relativeFilePath: string, name: string, line: number): string {
  return `${relativeFilePath}::${name}:${line}`;
}

function writeMessage(message: JsonRpcResponse): void {
  const body = JSON.stringify(message);
  const header = `Content-Length: ${Buffer.byteLength(body, "utf8")}\r\n\r\n`;
  process.stdout.write(header);
  process.stdout.write(body);
}

function respond(id: JsonRpcId, result?: unknown, error?: JsonRpcError): void {
  const response: JsonRpcResponse = {
    jsonrpc: "2.0",
    id,
  };

  if (error) {
    response.error = error;
  } else {
    response.result = result ?? null;
  }

  writeMessage(response);
}

function errorResponse(id: JsonRpcId, code: number, message: string, data?: unknown): void {
  respond(id, undefined, { code, message, data });
}

function handleRequest(request: JsonRpcRequest): void {
  const id = request.id ?? null;
  const method = request.method;

  if (request.jsonrpc !== "2.0") {
    errorResponse(id, -32600, "Invalid Request");
    return;
  }

  if (!method) {
    errorResponse(id, -32600, "Invalid Request");
    return;
  }

  if (method === "initialize") {
    if (request.id === undefined) {
      return;
    }

    respond(id, {
      protocolVersion: PROTOCOL_VERSION,
      capabilities: {
        semanticAnalysis: true,
        incrementalUpdates: false,
      },
      serverInfo: {
        name: "sentrux-ts-bridge",
        version: "0.1.0",
      },
    });
    return;
  }

  if (method === "ping") {
    if (request.id === undefined) {
      return;
    }

    respond(id, { ok: true });
    return;
  }

  if (method === "shutdown") {
    if (request.id === undefined) {
      return;
    }

    respond(id, null);
    return;
  }

  if (method === "analyze_projects") {
    if (request.id === undefined) {
      return;
    }

    const project = toProjectModel(request.params);
    if (!project) {
      errorResponse(id, -32602, "Invalid params", {
        expected: "ProjectModel",
      });
      return;
    }

    try {
      respond(id, analyzeProject(project));
    } catch (error) {
      errorResponse(id, -32001, "Semantic analysis failed", {
        message: error instanceof Error ? error.message : String(error),
      });
    }
    return;
  }

  if (method === "exit") {
    if (request.id !== undefined) {
      respond(id, null);
    }
    process.exit(0);
    return;
  }

  if (request.id === undefined) {
    return;
  }

  errorResponse(id, -32601, "Method not found", { method });
}

function analyzeProject(project: ProjectModel): SemanticSnapshot {
  const rootPath = path.resolve(project.root);
  const fileFacts: SemanticFileFact[] = [];
  const symbolFacts: SymbolFact[] = [];
  const readFacts: ReadFact[] = [];
  const writeFacts: WriteFact[] = [];
  const closedDomains: ClosedDomain[] = [];
  const closedDomainSites: ExhaustivenessSite[] = [];
  const transitionSites: TransitionSite[] = [];
  const seenFiles = new Set<string>();

  for (const tsconfigPath of project.tsconfig_paths) {
    const absoluteTsconfigPath = path.resolve(rootPath, tsconfigPath);
    const program = createProgramFromTsconfig(absoluteTsconfigPath);
    const checker = program.getTypeChecker();

    for (const sourceFile of program.getSourceFiles()) {
      if (shouldSkipSourceFile(sourceFile, rootPath, seenFiles)) {
        continue;
      }

      const context = analyzeSourceFile(rootPath, sourceFile, checker);
      seenFiles.add(sourceFile.fileName);
      fileFacts.push({
        path: context.relativePath,
        symbol_count: context.symbolFacts.length,
        write_count: context.writeFacts.length,
        closed_domain_count: context.closedDomains.length,
      });
      symbolFacts.push(...context.symbolFacts);
      readFacts.push(...context.readFacts);
      writeFacts.push(...context.writeFacts);
      closedDomains.push(...context.closedDomains);
      closedDomainSites.push(...context.closedDomainSites);
      transitionSites.push(...context.transitionSites);
    }
  }

  return {
    project,
    analyzed_files: fileFacts.length,
    capabilities: [
      "Symbols",
      "Reads",
      "Writes",
      "ClosedDomains",
      "ClosedDomainSites",
      "TransitionSites",
    ],
    files: fileFacts,
    symbols: symbolFacts,
    reads: readFacts,
    writes: writeFacts,
    closed_domains: closedDomains,
    closed_domain_sites: closedDomainSites,
    transition_sites: transitionSites,
  };
}

function createProgramFromTsconfig(tsconfigPath: string): ts.Program {
  const configFile = ts.readConfigFile(tsconfigPath, ts.sys.readFile);
  if (configFile.error) {
    throw new Error(formatDiagnostic(configFile.error));
  }

  const parsed = ts.parseJsonConfigFileContent(
    configFile.config,
    ts.sys,
    path.dirname(tsconfigPath),
  );
  if (parsed.errors.length > 0) {
    throw new Error(formatDiagnostics(parsed.errors));
  }

  return ts.createProgram({
    rootNames: parsed.fileNames,
    options: parsed.options,
  });
}

function shouldSkipSourceFile(
  sourceFile: ts.SourceFile,
  rootPath: string,
  seenFiles: Set<string>,
): boolean {
  if (sourceFile.isDeclarationFile) {
    return true;
  }

  const filePath = path.resolve(sourceFile.fileName);
  if (!normalizePath(filePath).startsWith(normalizePath(rootPath))) {
    return true;
  }

  return seenFiles.has(sourceFile.fileName);
}

function analyzeSourceFile(
  rootPath: string,
  sourceFile: ts.SourceFile,
  checker: ts.TypeChecker,
): FileAnalysisContext {
  const context: FileAnalysisContext = {
    rootPath,
    relativePath: relativePath(rootPath, sourceFile.fileName),
    sourceFile,
    checker,
    symbolFacts: [],
    readFacts: [],
    writeFacts: [],
    closedDomains: [],
    closedDomainSites: [],
    transitionSites: [],
  };

  function visit(node: ts.Node): void {
    collectSymbolFact(context, node);
    collectReadFacts(context, node);
    collectWriteFacts(context, node);
    collectClosedDomain(context, node);
    collectClosedDomainSite(context, node);
    collectTransitionSite(context, node);
    ts.forEachChild(node, visit);
  }

  ts.forEachChild(sourceFile, visit);

  return context;
}

function collectSymbolFact(context: FileAnalysisContext, node: ts.Node): void {
  if (ts.isFunctionDeclaration(node) && node.name && isTopLevelDeclaration(node)) {
    pushSymbolFact(context, node.name, "function");
    return;
  }

  if (ts.isClassDeclaration(node) && node.name && isTopLevelDeclaration(node)) {
    pushSymbolFact(context, node.name, "class");
    return;
  }

  if (ts.isInterfaceDeclaration(node) && isTopLevelDeclaration(node)) {
    pushSymbolFact(context, node.name, "interface");
    return;
  }

  if (ts.isTypeAliasDeclaration(node) && isTopLevelDeclaration(node)) {
    pushSymbolFact(context, node.name, "type_alias");
    return;
  }

  if (ts.isEnumDeclaration(node) && isTopLevelDeclaration(node)) {
    pushSymbolFact(context, node.name, "enum");
    return;
  }

  if (
    ts.isVariableDeclaration(node) &&
    ts.isIdentifier(node.name) &&
    isTopLevelVariableDeclaration(node)
  ) {
    pushSymbolFact(context, node.name, "variable");
    const objectLiteral = node.initializer
      ? unwrapObjectLiteralExpression(node.initializer)
      : null;
    if (objectLiteral) {
      collectObjectPropertySymbolFacts(context, node.name.text, objectLiteral);
    }
  }
}

function pushSymbolFact(
  context: FileAnalysisContext,
  identifier: ts.Identifier,
  kind: string,
): void {
  pushNamedSymbolFact(context, identifier.text, identifier, kind);
}

function pushNamedSymbolFact(
  context: FileAnalysisContext,
  name: string,
  node: ts.Node,
  kind: string,
): void {
  const line = lineOfNode(context.sourceFile, node);
  context.symbolFacts.push({
    id: symbolId(context.relativePath, name, line),
    path: context.relativePath,
    name,
    kind,
    line,
  });
}

function collectObjectPropertySymbolFacts(
  context: FileAnalysisContext,
  prefix: string,
  objectLiteral: ts.ObjectLiteralExpression,
): void {
  for (const property of objectLiteral.properties) {
    if (ts.isPropertyAssignment(property)) {
      const key = propertyNameText(property.name);
      if (!key) {
        continue;
      }

      const symbolName = `${prefix}.${key}`;
      pushNamedSymbolFact(context, symbolName, property.name, "property");
      const nestedObjectLiteral = unwrapObjectLiteralExpression(property.initializer);
      if (nestedObjectLiteral) {
        collectObjectPropertySymbolFacts(context, symbolName, nestedObjectLiteral);
      }
      continue;
    }

    if (!ts.isShorthandPropertyAssignment(property)) {
      continue;
    }

    const symbolName = `${prefix}.${property.name.text}`;
    pushNamedSymbolFact(context, symbolName, property.name, "property");
  }
}

function collectReadFacts(context: FileAnalysisContext, node: ts.Node): void {
  if (ts.isPropertyAccessExpression(node)) {
    const parent = node.parent;
    if (parent && ts.isPropertyAccessExpression(parent) && parent.expression === node) {
      return;
    }
    if (isWriteTarget(node)) {
      return;
    }

    const target = expressionPath(node);
    if (target) {
      context.readFacts.push({
        path: context.relativePath,
        symbol_name: target,
        read_kind: "property_access",
        line: lineOfNode(context.sourceFile, node),
      });
    }
    return;
  }

  if (ts.isElementAccessExpression(node)) {
    const parent = node.parent;
    if (parent && ts.isElementAccessExpression(parent) && parent.expression === node) {
      return;
    }
    if (isWriteTarget(node)) {
      return;
    }

    const target = expressionPath(node);
    if (target) {
      context.readFacts.push({
        path: context.relativePath,
        symbol_name: target,
        read_kind: "element_access",
        line: lineOfNode(context.sourceFile, node),
      });
    }
  }
}

function collectWriteFacts(context: FileAnalysisContext, node: ts.Node): void {
  if (ts.isBinaryExpression(node) && isAssignmentOperator(node.operatorToken.kind)) {
    const target = expressionName(node.left);
    if (target) {
      context.writeFacts.push({
        path: context.relativePath,
        symbol_name: target,
        write_kind: "assignment",
        line: lineOfNode(context.sourceFile, node),
      });
    }
    return;
  }

  if (
    (ts.isPrefixUnaryExpression(node) || ts.isPostfixUnaryExpression(node)) &&
    isMutationOperator(node.operator)
  ) {
    const target = expressionName(node.operand);
    if (target) {
      context.writeFacts.push({
        path: context.relativePath,
        symbol_name: target,
        write_kind: "mutation",
        line: lineOfNode(context.sourceFile, node),
      });
    }
    return;
  }

  if (ts.isCallExpression(node)) {
    const target = expressionName(node.expression);
    if (target === "setStore") {
      const storeTarget = setStoreTarget(node.arguments);
      context.writeFacts.push({
        path: context.relativePath,
        symbol_name: storeTarget ?? target,
        write_kind: "store_call",
        line: lineOfNode(context.sourceFile, node),
      });
    }
  }
}

function collectClosedDomain(context: FileAnalysisContext, node: ts.Node): void {
  if (ts.isTypeAliasDeclaration(node) && isTopLevelDeclaration(node)) {
    const variants = closedDomainVariantsFromTypeNode(context, node.type);
    if (variants.length > 1) {
      context.closedDomains.push({
        path: context.relativePath,
        symbol_name: node.name.text,
        variants,
        line: lineOfNode(context.sourceFile, node.name),
      });
    }
    return;
  }

  if (ts.isEnumDeclaration(node) && isTopLevelDeclaration(node)) {
    const variants: string[] = [];
    for (const member of node.members) {
      variants.push(member.name.getText(context.sourceFile));
    }
    if (variants.length > 0) {
      context.closedDomains.push({
        path: context.relativePath,
        symbol_name: node.name.text,
        variants,
        line: lineOfNode(context.sourceFile, node.name),
      });
    }
  }
}

function collectClosedDomainSite(context: FileAnalysisContext, node: ts.Node): void {
  if (ts.isSwitchStatement(node)) {
    collectSwitchExhaustivenessSite(context, node);
    return;
  }

  const initializerObjectLiteral =
    ts.isVariableDeclaration(node) && node.initializer
      ? unwrapObjectLiteralExpression(node.initializer)
      : null;
  if (
    ts.isVariableDeclaration(node) &&
    node.type &&
    initializerObjectLiteral
  ) {
    const recordInfo = recordDomainInfoFromTypeNode(context, node.type);
    if (!recordInfo) {
      return;
    }

    context.closedDomainSites.push({
      path: context.relativePath,
      domain_symbol_name: recordInfo.domainSymbolName,
      site_kind: "record",
      proof_kind: "Record",
      covered_variants: objectLiteralKeys(initializerObjectLiteral),
      line: lineOfNode(context.sourceFile, node.name),
    });
    return;
  }

  if (ts.isSatisfiesExpression(node)) {
    const objectLiteral = unwrapObjectLiteralExpression(node.expression);
    if (!objectLiteral) {
      return;
    }

    const recordInfo = recordDomainInfoFromTypeNode(context, node.type);
    if (!recordInfo) {
      return;
    }

    context.closedDomainSites.push({
      path: context.relativePath,
      domain_symbol_name: recordInfo.domainSymbolName,
      site_kind: "satisfies",
      proof_kind: "satisfies",
      covered_variants: objectLiteralKeys(objectLiteral),
      line: lineOfNode(context.sourceFile, objectLiteral),
    });
  }
}

function unwrapObjectLiteralExpression(
  expression: ts.Expression,
): ts.ObjectLiteralExpression | null {
  let current = expression;

  while (
    ts.isParenthesizedExpression(current) ||
    ts.isAsExpression(current) ||
    ts.isTypeAssertionExpression(current) ||
    ts.isSatisfiesExpression(current)
  ) {
    current = current.expression;
  }

  return ts.isObjectLiteralExpression(current) ? current : null;
}

function collectSwitchExhaustivenessSite(
  context: FileAnalysisContext,
  node: ts.SwitchStatement,
): void {
  const domainInfo = closedDomainInfoForExpression(context, node.expression);
  if (!domainInfo || domainInfo.variants.length <= 1) {
    return;
  }

  const coveredVariants: string[] = [];
  let proofKind = "switch";
  for (const clause of node.caseBlock.clauses) {
    if (ts.isDefaultClause(clause)) {
      if (clause.statements.some((statement) => containsAssertNever(statement))) {
        proofKind = "assertNever";
      }
      continue;
    }

    const variant = literalExpressionText(clause.expression);
    if (variant) {
      coveredVariants.push(variant);
    }
  }

  if (proofKind !== "assertNever" && switchHasTrailingAssertNever(node)) {
    proofKind = "assertNever";
  }

  context.closedDomainSites.push({
    path: context.relativePath,
    domain_symbol_name: domainInfo.domainSymbolName,
    site_kind: "switch",
    proof_kind: proofKind,
    covered_variants: coveredVariants,
    line: lineOfNode(context.sourceFile, node.expression),
  });
}

function collectTransitionSite(context: FileAnalysisContext, node: ts.Node): void {
  if (ts.isVariableDeclaration(node)) {
    collectRecordTransitionSites(context, node);
  }

  if (ts.isSwitchStatement(node)) {
    collectSwitchTransitionSites(context, node);
    return;
  }

  if (
    ts.isIfStatement(node) &&
    !(ts.isIfStatement(node.parent) && node.parent.elseStatement === node)
  ) {
    collectIfTransitionSites(context, node);
  }
}

function collectRecordTransitionSites(
  context: FileAnalysisContext,
  node: ts.VariableDeclaration,
): void {
  const initializer = node.initializer;
  if (!initializer) {
    return;
  }

  const objectLiteral = unwrapObjectLiteralExpression(initializer);
  if (!objectLiteral) {
    return;
  }

  const recordInfo = transitionRecordInfoForVariableDeclaration(context, node);
  if (!recordInfo) {
    return;
  }

  const recordLine = lineOfNode(context.sourceFile, node.name);
  const groupId = `${context.relativePath}:${recordLine}:${recordInfo.domainInfo.domainSymbolName}:record`;
  const groupSites: TransitionSite[] = [];

  for (const property of objectLiteral.properties) {
    const sourceVariant = transitionSourceVariantFromObjectProperty(
      property,
      recordInfo.domainInfo.variants,
    );
    if (!sourceVariant) {
      continue;
    }

    const targetVariants = transitionTargetsFromObjectProperty(
      property,
      recordInfo.targetVariants,
    );
    groupSites.push({
      path: context.relativePath,
      domain_symbol_name: recordInfo.domainInfo.domainSymbolName,
      group_id: groupId,
      transition_kind: "record_entry",
      source_variant: sourceVariant,
      target_variants: targetVariants,
      line: lineOfNode(context.sourceFile, property),
    });
  }

  if (groupSites.length > 0) {
    context.transitionSites.push(...groupSites);
  }
}

function transitionRecordInfoForVariableDeclaration(
  context: FileAnalysisContext,
  node: ts.VariableDeclaration,
): {
  domainInfo: ClosedDomainInfo;
  targetVariants: string[];
} | null {
  if (node.type) {
    const recordInfo = transitionRecordInfoFromTypeNode(context, node.type);
    if (recordInfo) {
      return recordInfo;
    }
  }

  const initializer = node.initializer;
  if (!initializer || !ts.isSatisfiesExpression(initializer)) {
    return null;
  }

  return transitionRecordInfoFromTypeNode(context, initializer.type);
}

function transitionRecordInfoFromTypeNode(
  context: FileAnalysisContext,
  typeNode: ts.TypeNode,
): {
  domainInfo: ClosedDomainInfo;
  targetVariants: string[];
} | null {
  if (!ts.isTypeReferenceNode(typeNode)) {
    return null;
  }

  const typeName = typeNode.typeName.getText(context.sourceFile);
  if (
    typeName !== "Record" ||
    !typeNode.typeArguments ||
    typeNode.typeArguments.length < 2
  ) {
    return null;
  }

  const sourceInfo = closedDomainInfoForTypeNode(context, typeNode.typeArguments[0]);
  const targetInfo = closedDomainInfoForTypeNode(context, typeNode.typeArguments[1]);
  if (!sourceInfo || !targetInfo || sourceInfo.variants.length <= 1) {
    return null;
  }

  if (sourceInfo.domainSymbolName !== targetInfo.domainSymbolName) {
    return null;
  }

  return {
    domainInfo: sourceInfo,
    targetVariants: targetInfo.variants,
  };
}

function transitionSourceVariantFromObjectProperty(
  property: ts.ObjectLiteralElementLike,
  allowedVariants: readonly string[],
): string | null {
  if (ts.isPropertyAssignment(property)) {
    return transitionVariantTextFromPropertyName(property.name, allowedVariants);
  }

  if (ts.isShorthandPropertyAssignment(property)) {
    return allowedVariants.includes(property.name.text) ? property.name.text : null;
  }

  return null;
}

function transitionTargetsFromObjectProperty(
  property: ts.ObjectLiteralElementLike,
  allowedVariants: readonly string[],
): string[] {
  if (ts.isPropertyAssignment(property)) {
    return collectTransitionTargetVariantsFromExpression(
      property.initializer,
      allowedVariants,
    );
  }

  if (
    ts.isShorthandPropertyAssignment(property) &&
    allowedVariants.includes(property.name.text)
  ) {
    return [property.name.text];
  }

  return [];
}

function collectSwitchTransitionSites(
  context: FileAnalysisContext,
  node: ts.SwitchStatement,
): void {
  const domainInfo = closedDomainInfoForExpression(context, node.expression);
  if (!domainInfo || domainInfo.variants.length <= 1) {
    return;
  }

  const switchLine = lineOfNode(context.sourceFile, node.expression);
  const groupId = `${context.relativePath}:${switchLine}:${domainInfo.domainSymbolName}`;
  const groupSites: TransitionSite[] = [];
  let hasPotentialIntent = false;
  for (const clause of node.caseBlock.clauses) {
    if (!ts.isCaseClause(clause)) {
      continue;
    }

    const sourceVariant = transitionVariantText(clause.expression, domainInfo.variants);
    if (!sourceVariant || !domainInfo.variants.includes(sourceVariant)) {
      continue;
    }

    groupSites.push({
      path: context.relativePath,
      domain_symbol_name: domainInfo.domainSymbolName,
      group_id: groupId,
      transition_kind: "switch_case",
      source_variant: sourceVariant,
      target_variants: collectTransitionTargetVariants(
        clause.statements,
        domainInfo.variants,
      ),
      line: lineOfNode(context.sourceFile, clause.expression),
    });
    hasPotentialIntent =
      hasPotentialIntent ||
      hasPotentialTransitionIntent(context, clause.statements, domainInfo);
  }

  if (
    groupSites.some((site) => site.target_variants.length > 0) ||
    (hasPotentialIntent && groupSites.length > 0)
  ) {
    context.transitionSites.push(...groupSites);
  }
}

function collectIfTransitionSites(
  context: FileAnalysisContext,
  node: ts.IfStatement,
): void {
  const groupSites: TransitionSite[] = [];
  let current: ts.IfStatement | undefined = node;
  let domainInfo: ClosedDomainInfo | null = null;
  let subjectPath: string | null = null;
  const matchedVariants = new Set<string>();
  const groupLine = lineOfNode(context.sourceFile, node.expression);
  let hasPotentialIntent = false;

  while (current) {
    const match = ifConditionVariant(
      context,
      current.expression,
      subjectPath,
      domainInfo,
    );
    if (!match) {
      return;
    }

    if (!domainInfo) {
      domainInfo = match.domainInfo;
      subjectPath = match.subjectPath;
    } else if (
      subjectPath !== match.subjectPath ||
      domainInfo.domainSymbolName !== match.domainInfo.domainSymbolName
    ) {
      return;
    }

    matchedVariants.add(match.sourceVariant);
    groupSites.push({
      path: context.relativePath,
      domain_symbol_name: match.domainInfo.domainSymbolName,
      group_id: `${context.relativePath}:${groupLine}:${match.domainInfo.domainSymbolName}`,
      transition_kind: "if_branch",
      source_variant: match.sourceVariant,
      target_variants: collectTransitionTargetVariants(
        statementsOf(current.thenStatement),
        match.domainInfo.variants,
      ),
      line: lineOfNode(context.sourceFile, current.expression),
    });
    hasPotentialIntent =
      hasPotentialIntent ||
      hasPotentialTransitionIntent(
        context,
        statementsOf(current.thenStatement),
        match.domainInfo,
      );

    if (!current.elseStatement) {
      current = undefined;
      continue;
    }

    if (ts.isIfStatement(current.elseStatement)) {
      current = current.elseStatement;
      continue;
    }

    if (domainInfo) {
      const remainingVariants = domainInfo.variants.filter(
        (variant) => !matchedVariants.has(variant),
      );
      const elseTargets = collectTransitionTargetVariants(
        statementsOf(current.elseStatement),
        domainInfo.variants,
      );
      hasPotentialIntent =
        hasPotentialIntent ||
        hasPotentialTransitionIntent(
          context,
          statementsOf(current.elseStatement),
          domainInfo,
        );
      for (const sourceVariant of remainingVariants) {
        groupSites.push({
          path: context.relativePath,
          domain_symbol_name: domainInfo.domainSymbolName,
          group_id: `${context.relativePath}:${groupLine}:${domainInfo.domainSymbolName}`,
          transition_kind: "if_else",
          source_variant: sourceVariant,
          target_variants: elseTargets,
          line: lineOfNode(context.sourceFile, current.elseStatement),
        });
      }
    }

    current = undefined;
  }

  if (
    groupSites.some((site) => site.target_variants.length > 0) ||
    (hasPotentialIntent && groupSites.length > 0)
  ) {
    context.transitionSites.push(...groupSites);
  }
}

function ifConditionVariant(
  context: FileAnalysisContext,
  expression: ts.Expression,
  expectedSubjectPath: string | null,
  expectedDomainInfo: ClosedDomainInfo | null,
): {
  domainInfo: ClosedDomainInfo;
  subjectPath: string;
  sourceVariant: string;
} | null {
  const current = unwrapTransitionExpression(expression);
  if (
    !ts.isBinaryExpression(current) ||
    !isTransitionComparisonOperator(current.operatorToken.kind)
  ) {
    return null;
  }

  const leftPath = expressionPath(current.left);
  const rightPath = expressionPath(current.right);

  if (leftPath) {
    const domainInfo =
      expectedSubjectPath === leftPath && expectedDomainInfo
        ? expectedDomainInfo
        : closedDomainInfoForExpression(context, current.left);
    const sourceVariant = domainInfo
      ? sourceVariantForCondition(
          domainInfo.variants,
          transitionVariantText(current.right, domainInfo.variants),
          current.operatorToken.kind,
        )
      : null;
    if (domainInfo && sourceVariant) {
      return {
        domainInfo,
        subjectPath: leftPath,
        sourceVariant,
      };
    }
  }

  if (rightPath) {
    const domainInfo =
      expectedSubjectPath === rightPath && expectedDomainInfo
        ? expectedDomainInfo
        : closedDomainInfoForExpression(context, current.right);
    const sourceVariant = domainInfo
      ? sourceVariantForCondition(
          domainInfo.variants,
          transitionVariantText(current.left, domainInfo.variants),
          current.operatorToken.kind,
        )
      : null;
    if (domainInfo && sourceVariant) {
      return {
        domainInfo,
        subjectPath: rightPath,
        sourceVariant,
      };
    }
  }

  return null;
}

function isTransitionComparisonOperator(kind: ts.SyntaxKind): boolean {
  return (
    kind === ts.SyntaxKind.EqualsEqualsEqualsToken ||
    kind === ts.SyntaxKind.EqualsEqualsToken ||
    kind === ts.SyntaxKind.ExclamationEqualsEqualsToken ||
    kind === ts.SyntaxKind.ExclamationEqualsToken
  );
}

function sourceVariantForCondition(
  variants: readonly string[],
  comparedVariant: string | null,
  operatorKind: ts.SyntaxKind,
): string | null {
  if (!comparedVariant) {
    return null;
  }

  if (
    operatorKind === ts.SyntaxKind.EqualsEqualsEqualsToken ||
    operatorKind === ts.SyntaxKind.EqualsEqualsToken
  ) {
    return comparedVariant;
  }

  const remainingVariants = variants.filter((variant) => variant !== comparedVariant);
  return remainingVariants.length === 1 ? remainingVariants[0] : null;
}

function statementsOf(statement: ts.Statement): readonly ts.Statement[] {
  if (ts.isBlock(statement)) {
    return statement.statements;
  }

  return [statement];
}

function collectTransitionTargetVariants(
  statements: readonly ts.Statement[],
  allowedVariants: readonly string[],
): string[] {
  const variants = new Set<string>();

  function visit(node: ts.Node): void {
    if (ts.isReturnStatement(node) && node.expression) {
      for (const variant of collectTransitionTargetVariantsFromExpression(
        node.expression,
        allowedVariants,
      )) {
        variants.add(variant);
      }
    } else if (ts.isBinaryExpression(node) && isAssignmentOperator(node.operatorToken.kind)) {
      for (const variant of collectTransitionTargetVariantsFromExpression(
        node.right,
        allowedVariants,
      )) {
        variants.add(variant);
      }
    }

    ts.forEachChild(node, visit);
  }

  for (const statement of statements) {
    visit(statement);
  }

  return [...variants];
}

function collectTransitionTargetVariantsFromExpression(
  expression: ts.Expression,
  allowedVariants: readonly string[],
): string[] {
  const variants = new Set<string>();

  function recordExpression(currentExpression: ts.Expression): void {
    const current = unwrapTransitionExpression(currentExpression);
    const targetVariant = transitionVariantText(current, allowedVariants);
    if (targetVariant && allowedVariants.includes(targetVariant)) {
      variants.add(targetVariant);
      return;
    }

    if (ts.isObjectLiteralExpression(current)) {
      for (const property of current.properties) {
        if (ts.isPropertyAssignment(property)) {
          recordExpression(property.initializer);
        } else if (
          ts.isShorthandPropertyAssignment(property) &&
          allowedVariants.includes(property.name.text)
        ) {
          variants.add(property.name.text);
        }
      }
      return;
    }

    if (ts.isConditionalExpression(current)) {
      recordExpression(current.whenTrue);
      recordExpression(current.whenFalse);
      return;
    }

    if (ts.isArrayLiteralExpression(current)) {
      for (const element of current.elements) {
        if (ts.isExpression(element)) {
          recordExpression(element);
        }
      }
    }
  }

  recordExpression(expression);
  return [...variants];
}

function hasPotentialTransitionIntent(
  context: FileAnalysisContext,
  statements: readonly ts.Statement[],
  domainInfo: ClosedDomainInfo,
): boolean {
  function domainTyped(expression: ts.Expression): boolean {
    const expressionDomainInfo = closedDomainInfoForExpression(context, expression);
    return expressionDomainInfo?.domainSymbolName === domainInfo.domainSymbolName;
  }

  function visit(node: ts.Node): boolean {
    if (ts.isReturnStatement(node) && node.expression && domainTyped(node.expression)) {
      return true;
    }

    if (ts.isBinaryExpression(node) && isAssignmentOperator(node.operatorToken.kind)) {
      return domainTyped(node.right);
    }

    if (
      ts.isVariableDeclaration(node) &&
      node.initializer &&
      domainTyped(node.initializer)
    ) {
      return true;
    }

    return ts.forEachChild(node, visit) ?? false;
  }

  for (const statement of statements) {
    if (visit(statement)) {
      return true;
    }
  }

  return false;
}

function unwrapTransitionExpression(expression: ts.Expression): ts.Expression {
  let current = expression;

  while (
    ts.isParenthesizedExpression(current) ||
    ts.isAsExpression(current) ||
    ts.isTypeAssertionExpression(current) ||
    ts.isSatisfiesExpression(current) ||
    ts.isNonNullExpression(current)
  ) {
    current = current.expression;
  }

  return current;
}

function transitionVariantText(
  expression: ts.Expression,
  allowedVariants: readonly string[],
): string | null {
  const current = unwrapTransitionExpression(expression);
  const literalVariant = literalExpressionText(current);
  if (literalVariant) {
    return literalVariant;
  }

  if (ts.isIdentifier(current) && allowedVariants.includes(current.text)) {
    return current.text;
  }

  if (
    ts.isPropertyAccessExpression(current) &&
    allowedVariants.includes(current.name.text)
  ) {
    return current.name.text;
  }

  if (
    ts.isElementAccessExpression(current) &&
    current.argumentExpression &&
    (ts.isStringLiteral(current.argumentExpression) ||
      ts.isNumericLiteral(current.argumentExpression)) &&
    allowedVariants.includes(current.argumentExpression.text)
  ) {
    return current.argumentExpression.text;
  }

  return null;
}

function recordDomainInfoFromTypeNode(
  context: FileAnalysisContext,
  typeNode: ts.TypeNode,
): { domainSymbolName: string } | null {
  if (!ts.isTypeReferenceNode(typeNode)) {
    return null;
  }

  const typeName = typeNode.typeName.getText(context.sourceFile);
  if (
    typeName !== "Record" ||
    !typeNode.typeArguments ||
    typeNode.typeArguments.length < 1
  ) {
    return null;
  }

  const domainInfo = closedDomainInfoForTypeNode(context, typeNode.typeArguments[0]);
  if (!domainInfo || domainInfo.variants.length <= 1) {
    return null;
  }

  return {
    domainSymbolName: domainInfo.domainSymbolName,
  };
}

function closedDomainInfoForExpression(
  context: FileAnalysisContext,
  expression: ts.Expression,
): ClosedDomainInfo | null {
  const type = context.checker.getTypeAtLocation(expression);
  const directInfo = closedDomainInfoFromType(context, type);
  if (directInfo) {
    return directInfo;
  }

  const discriminantInfo = discriminantAccessInfo(expression);
  if (!discriminantInfo) {
    return null;
  }

  const baseType = context.checker.getTypeAtLocation(discriminantInfo.baseExpression);
  return closedDomainInfoFromType(context, baseType, discriminantInfo.propertyName);
}

function closedDomainInfoForTypeNode(
  context: FileAnalysisContext,
  typeNode: ts.TypeNode,
): ClosedDomainInfo | null {
  const type = context.checker.getTypeFromTypeNode(typeNode);
  return closedDomainInfoFromType(context, type);
}

function closedDomainInfoFromType(
  context: FileAnalysisContext,
  type: ts.Type,
  preferredDiscriminant?: string,
): ClosedDomainInfo | null {
  const variants = literalVariantsFromType(type);
  if (variants.length > 1) {
    const domainSymbolName = domainSymbolNameFromType(type);
    if (domainSymbolName) {
      return {
        domainSymbolName,
        variants,
      };
    }
  }

  const discriminatedInfo = discriminatedUnionInfoFromType(
    context,
    type,
    preferredDiscriminant,
  );
  if (!discriminatedInfo) {
    return null;
  }

  const domainSymbolName = domainSymbolNameFromType(type);
  if (!domainSymbolName) {
    return null;
  }

  return {
    domainSymbolName,
    variants: discriminatedInfo.variants,
  };
}

function closedDomainVariantsFromTypeNode(
  context: FileAnalysisContext,
  typeNode: ts.TypeNode,
): string[] {
  const variants = literalUnionVariants(typeNode);
  if (variants.length > 1) {
    return variants;
  }

  const type = context.checker.getTypeFromTypeNode(typeNode);
  return discriminatedUnionInfoFromType(context, type)?.variants ?? [];
}

function domainSymbolNameFromType(type: ts.Type): string | null {
  const aliasSymbol = type.aliasSymbol;
  if (aliasSymbol) {
    return aliasSymbol.getName();
  }

  const typeSymbol = type.getSymbol();
  if (typeSymbol && typeSymbol.getName() !== "__type") {
    return typeSymbol.getName();
  }

  return null;
}

function discriminantAccessInfo(
  expression: ts.Expression,
): { baseExpression: ts.Expression; propertyName: string } | null {
  if (ts.isPropertyAccessExpression(expression)) {
    return {
      baseExpression: expression.expression,
      propertyName: expression.name.text,
    };
  }

  if (
    ts.isElementAccessExpression(expression) &&
    expression.argumentExpression &&
    ts.isStringLiteral(expression.argumentExpression)
  ) {
    return {
      baseExpression: expression.expression,
      propertyName: expression.argumentExpression.text,
    };
  }

  return null;
}

function discriminatedUnionInfoFromType(
  context: FileAnalysisContext,
  type: ts.Type,
  preferredDiscriminant?: string,
): { variants: string[] } | null {
  if (!type.isUnion()) {
    return null;
  }

  const candidateNames = discriminantCandidateNames(type, preferredDiscriminant);
  for (const propertyName of candidateNames) {
    const variants: string[] = [];
    let isValid = true;

    for (const member of type.types) {
      const property = member.getProperty(propertyName);
      const declaration = property?.valueDeclaration ?? property?.declarations?.[0];
      if (!property || !declaration) {
        isValid = false;
        break;
      }

      const propertyType = context.checker.getTypeOfSymbolAtLocation(property, declaration);
      const variant = literalVariantFromType(propertyType);
      if (!variant) {
        isValid = false;
        break;
      }

      variants.push(variant);
    }

    if (isValid && variants.length > 1 && new Set(variants).size === variants.length) {
      return { variants };
    }
  }

  return null;
}

function discriminantCandidateNames(
  type: ts.UnionType,
  preferredDiscriminant?: string,
): string[] {
  const commonNames = commonPropertyNames(type);
  const prioritized = [preferredDiscriminant, "kind", "type", "status", "state"].filter(
    (value): value is string => Boolean(value),
  );
  const candidates: string[] = [];

  for (const propertyName of prioritized) {
    if (commonNames.has(propertyName) && !candidates.includes(propertyName)) {
      candidates.push(propertyName);
    }
  }

  for (const propertyName of commonNames) {
    if (!candidates.includes(propertyName)) {
      candidates.push(propertyName);
    }
  }

  return candidates;
}

function commonPropertyNames(type: ts.UnionType): Set<string> {
  const members = type.types;
  if (members.length === 0) {
    return new Set();
  }

  let common = new Set(members[0].getProperties().map((property) => property.getName()));
  for (const member of members.slice(1)) {
    const names = new Set(member.getProperties().map((property) => property.getName()));
    common = new Set([...common].filter((name) => names.has(name)));
  }

  return common;
}

function literalVariantsFromType(type: ts.Type): string[] {
  if (type.isUnion()) {
    const variants: string[] = [];
    for (const member of type.types) {
      const variant = literalVariantFromType(member);
      if (!variant) {
        return [];
      }
      variants.push(variant);
    }
    return variants;
  }

  const enumVariants = enumVariantsFromType(type);
  if (enumVariants.length > 0) {
    return enumVariants;
  }

  const literal = literalVariantFromType(type);
  return literal ? [literal] : [];
}

function enumVariantsFromType(type: ts.Type): string[] {
  const symbol = type.getSymbol();
  if (!symbol || !(symbol.flags & ts.SymbolFlags.Enum)) {
    return [];
  }

  const declaration = symbol.declarations?.find(ts.isEnumDeclaration);
  if (!declaration) {
    return [];
  }

  return declaration.members.map((member) =>
    member.name.getText(declaration.getSourceFile()),
  );
}

function literalVariantFromType(type: ts.Type): string | null {
  if (type.flags & ts.TypeFlags.StringLiteral) {
    return (type as ts.StringLiteralType).value;
  }
  if (type.flags & ts.TypeFlags.NumberLiteral) {
    return String((type as ts.NumberLiteralType).value);
  }
  if (type.flags & ts.TypeFlags.BooleanLiteral) {
    return (type as { intrinsicName?: string }).intrinsicName ?? null;
  }

  return null;
}

function objectLiteralKeys(node: ts.ObjectLiteralExpression): string[] {
  const keys: string[] = [];

  for (const property of node.properties) {
    if (!ts.isPropertyAssignment(property) && !ts.isShorthandPropertyAssignment(property)) {
      continue;
    }

    const key = propertyNameText(property.name);
    if (key) {
      keys.push(key);
    }
  }

  return keys;
}

function propertyNameText(name: ts.PropertyName): string | null {
  if (ts.isIdentifier(name) || ts.isStringLiteral(name) || ts.isNumericLiteral(name)) {
    return name.text;
  }

  return null;
}

function transitionVariantTextFromPropertyName(
  name: ts.PropertyName,
  allowedVariants: readonly string[],
): string | null {
  const propertyText = propertyNameText(name);
  if (propertyText && allowedVariants.includes(propertyText)) {
    return propertyText;
  }

  if (
    ts.isComputedPropertyName(name) &&
    ts.isExpression(name.expression)
  ) {
    return transitionVariantText(name.expression, allowedVariants);
  }

  return null;
}

function literalExpressionText(expression: ts.Expression): string | null {
  if (ts.isStringLiteral(expression) || ts.isNumericLiteral(expression)) {
    return expression.text;
  }
  if (
    expression.kind === ts.SyntaxKind.TrueKeyword ||
    expression.kind === ts.SyntaxKind.FalseKeyword
  ) {
    return expression.getText();
  }

  return null;
}

function containsAssertNever(node: ts.Node): boolean {
  let found = false;

  function visit(child: ts.Node): void {
    if (found) {
      return;
    }
    if (
      ts.isCallExpression(child) &&
      ts.isIdentifier(child.expression) &&
      child.expression.text === "assertNever"
    ) {
      found = true;
      return;
    }
    ts.forEachChild(child, visit);
  }

  visit(node);
  return found;
}

function switchHasTrailingAssertNever(node: ts.SwitchStatement): boolean {
  const parent = node.parent;
  if (!parent || (!ts.isBlock(parent) && !ts.isSourceFile(parent))) {
    return false;
  }

  const statements = parent.statements;
  const index = statements.findIndex((statement) => statement === node);
  if (index < 0 || index + 1 >= statements.length) {
    return false;
  }

  const nextStatement = statements[index + 1];
  const subjects = switchAssertNeverSubjects(node.expression);
  return statementContainsAssertNeverForSubjects(nextStatement, subjects);
}

function switchAssertNeverSubjects(expression: ts.Expression): Set<string> {
  const subjects = new Set<string>([expression.getText()]);
  const discriminantInfo = discriminantAccessInfo(expression);
  if (discriminantInfo) {
    subjects.add(discriminantInfo.baseExpression.getText());
  }
  return subjects;
}

function statementContainsAssertNeverForSubjects(
  node: ts.Node,
  subjects: Set<string>,
): boolean {
  let found = false;

  function visit(child: ts.Node): void {
    if (found) {
      return;
    }
    if (
      ts.isCallExpression(child) &&
      ts.isIdentifier(child.expression) &&
      child.expression.text === "assertNever"
    ) {
      const firstArgument = child.arguments[0];
      if (firstArgument && subjects.has(firstArgument.getText())) {
        found = true;
      }
      return;
    }
    ts.forEachChild(child, visit);
  }

  visit(node);
  return found;
}

function isTopLevelDeclaration(node: ts.Node): boolean {
  return node.parent !== undefined && ts.isSourceFile(node.parent);
}

function isTopLevelVariableDeclaration(node: ts.VariableDeclaration): boolean {
  const declarationList = node.parent;
  if (!declarationList || !ts.isVariableDeclarationList(declarationList)) {
    return false;
  }

  const statement = declarationList.parent;
  if (!statement || !ts.isVariableStatement(statement)) {
    return false;
  }

  return statement.parent !== undefined && ts.isSourceFile(statement.parent);
}

function literalUnionVariants(typeNode: ts.TypeNode): string[] {
  if (!ts.isUnionTypeNode(typeNode)) {
    return [];
  }

  const variants: string[] = [];
  for (const member of typeNode.types) {
    const variant = literalTypeText(member);
    if (!variant) {
      return [];
    }
    variants.push(variant);
  }

  return variants;
}

function literalTypeText(typeNode: ts.TypeNode): string | null {
  if (!ts.isLiteralTypeNode(typeNode)) {
    return null;
  }

  if (ts.isStringLiteral(typeNode.literal) || ts.isNumericLiteral(typeNode.literal)) {
    return typeNode.literal.text;
  }

  if (
    typeNode.literal.kind === ts.SyntaxKind.TrueKeyword ||
    typeNode.literal.kind === ts.SyntaxKind.FalseKeyword
  ) {
    return typeNode.literal.getText();
  }

  return null;
}

function expressionName(expression: ts.Expression): string | null {
  if (ts.isIdentifier(expression)) {
    return expression.text;
  }

  if (ts.isPropertyAccessExpression(expression)) {
    return expression.name.text;
  }

  if (ts.isElementAccessExpression(expression)) {
    return expression.argumentExpression?.getText() ?? null;
  }

  return null;
}

function expressionPath(expression: ts.Expression): string | null {
  if (ts.isIdentifier(expression)) {
    return expression.text;
  }

  if (ts.isPropertyAccessExpression(expression)) {
    const base = expressionPath(expression.expression);
    if (!base) {
      return null;
    }
    return `${base}.${expression.name.text}`;
  }

  if (
    ts.isElementAccessExpression(expression) &&
    expression.argumentExpression &&
    (ts.isStringLiteral(expression.argumentExpression) ||
      ts.isNumericLiteral(expression.argumentExpression))
  ) {
    const base = expressionPath(expression.expression);
    if (!base) {
      return null;
    }
    return `${base}.${expression.argumentExpression.text}`;
  }

  return null;
}

function isWriteTarget(node: ts.Expression): boolean {
  const parent = node.parent;
  if (!parent) {
    return false;
  }

  if (ts.isBinaryExpression(parent) && parent.left === node) {
    return isAssignmentOperator(parent.operatorToken.kind);
  }

  if (
    (ts.isPrefixUnaryExpression(parent) || ts.isPostfixUnaryExpression(parent)) &&
    parent.operand === node
  ) {
    return isMutationOperator(parent.operator);
  }

  return false;
}

function setStoreTarget(argumentsList: readonly ts.Expression[]): string | null {
  if (argumentsList.length === 0) {
    return null;
  }

  const segments: string[] = [];
  const selectorArguments = argumentsList.slice(
    0,
    argumentsList.length > 1 ? argumentsList.length - 1 : argumentsList.length,
  );

  for (const argument of selectorArguments) {
    if (ts.isStringLiteral(argument) || ts.isNumericLiteral(argument)) {
      segments.push(argument.text);
      continue;
    }

    if (ts.isIdentifier(argument) || ts.isPropertyAccessExpression(argument)) {
      segments.push("*");
      continue;
    }

    break;
  }

  if (segments.length === 0) {
    return null;
  }

  return `store.${segments.join(".")}`;
}

function isAssignmentOperator(kind: ts.SyntaxKind): boolean {
  switch (kind) {
    case ts.SyntaxKind.EqualsToken:
    case ts.SyntaxKind.PlusEqualsToken:
    case ts.SyntaxKind.MinusEqualsToken:
    case ts.SyntaxKind.AsteriskEqualsToken:
    case ts.SyntaxKind.AsteriskAsteriskEqualsToken:
    case ts.SyntaxKind.SlashEqualsToken:
    case ts.SyntaxKind.PercentEqualsToken:
    case ts.SyntaxKind.AmpersandEqualsToken:
    case ts.SyntaxKind.BarEqualsToken:
    case ts.SyntaxKind.CaretEqualsToken:
    case ts.SyntaxKind.LessThanLessThanEqualsToken:
    case ts.SyntaxKind.GreaterThanGreaterThanEqualsToken:
    case ts.SyntaxKind.GreaterThanGreaterThanGreaterThanEqualsToken:
    case ts.SyntaxKind.QuestionQuestionEqualsToken:
    case ts.SyntaxKind.BarBarEqualsToken:
    case ts.SyntaxKind.AmpersandAmpersandEqualsToken:
      return true;
    default:
      return false;
  }
}

function isMutationOperator(kind: ts.SyntaxKind): boolean {
  return kind === ts.SyntaxKind.PlusPlusToken || kind === ts.SyntaxKind.MinusMinusToken;
}

function formatDiagnostic(diagnostic: ts.Diagnostic): string {
  return ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n");
}

function formatDiagnostics(diagnostics: readonly ts.Diagnostic[]): string {
  return diagnostics.map(formatDiagnostic).join("\n");
}

function parseHeaders(headerText: string): Map<string, string> {
  const headers = new Map<string, string>();

  for (const line of headerText.split("\r\n")) {
    const separator = line.indexOf(":");
    if (separator <= 0) {
      continue;
    }

    const name = line.slice(0, separator).trim().toLowerCase();
    const value = line.slice(separator + 1).trim();
    headers.set(name, value);
  }

  return headers;
}

function main(): void {
  let buffer = Buffer.alloc(0);

  process.stdin.on("data", function handleData(chunk: Buffer): void {
    buffer = Buffer.concat([buffer, chunk]);

    while (true) {
      const headerEnd = buffer.indexOf("\r\n\r\n");
      if (headerEnd < 0) {
        return;
      }

      const headerText = buffer.subarray(0, headerEnd).toString("utf8");
      const headers = parseHeaders(headerText);
      const contentLength = headers.get("content-length");

      if (!contentLength) {
        buffer = buffer.subarray(headerEnd + 4);
        continue;
      }

      const bodyLength = Number.parseInt(contentLength, 10);
      if (!Number.isFinite(bodyLength) || bodyLength < 0) {
        buffer = Buffer.alloc(0);
        errorResponse(null, -32600, "Invalid Request");
        continue;
      }

      const messageStart = headerEnd + 4;
      const messageEnd = messageStart + bodyLength;
      if (buffer.length < messageEnd) {
        return;
      }

      const bodyText = buffer.subarray(messageStart, messageEnd).toString("utf8");
      buffer = buffer.subarray(messageEnd);

      let parsed: unknown;
      try {
        parsed = JSON.parse(bodyText);
      } catch (error) {
        errorResponse(null, -32700, "Parse error", {
          message: error instanceof Error ? error.message : String(error),
        });
        continue;
      }

      const request = toRequest(parsed);
      if (!request) {
        errorResponse(null, -32600, "Invalid Request");
        continue;
      }

      handleRequest(request);
    }
  });

  process.stdin.on("end", function handleEnd(): void {
    process.exit(0);
  });
}

main();
