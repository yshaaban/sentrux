import path from "node:path";
import ts from "typescript";

import type { FileAnalysisContext } from "./analysis-types.js";
import type {
  ClosedDomain,
  ExhaustivenessSite,
  ProjectModel,
  ReadFact,
  SemanticFileFact,
  SemanticSnapshot,
  SymbolFact,
  TransitionSite,
  WriteFact,
} from "./types.js";
import { collectClosedDomain, collectClosedDomainSite } from "./analysis-closed-domains.js";
import { collectTransitionSite } from "./analysis-transitions.js";
import {
  createProgramFromTsconfig,
  isTopLevelDeclaration,
  isTopLevelVariableDeclaration,
  lineOfNode,
  propertyNameText,
  relativePath,
  shouldSkipSourceFile,
  symbolId,
  unwrapObjectLiteralExpression,
} from "./analysis-utils.js";

export function analyzeProject(project: ProjectModel): SemanticSnapshot {
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
