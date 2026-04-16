import ts from "typescript";

import type { ClosedDomainInfo, FileAnalysisContext } from "./analysis-types.js";
import { ExhaustivenessProofKind as ExhaustivenessProofKindType } from "./types.js";
import type { TransitionSite } from "./types.js";
import { TransitionKind } from "./types.js";
import {
  expressionPath,
  isAssignmentOperator,
  lineOfNode,
  literalExpressionText,
  propertyNameText,
  unwrapObjectLiteralExpression,
} from "./analysis-utils.js";
import { closedDomainInfoForExpression, closedDomainInfoForTypeNode } from "./analysis-closed-domains.js";

export function collectTransitionSite(context: FileAnalysisContext, node: ts.Node): void {
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
      transition_kind: TransitionKind.RecordEntry,
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
      transition_kind: TransitionKind.SwitchCase,
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
    const match = ifConditionVariant(context, current.expression, subjectPath, domainInfo);
    if (!match) {
      return;
    }

    if (!matchesIfTransitionGroup(domainInfo, subjectPath, match)) {
      return;
    }
    if (!domainInfo) {
      ({ domainInfo, subjectPath } = initializeIfTransitionGroup(match));
    }

    matchedVariants.add(match.sourceVariant);
    const thenStatements = statementsOf(current.thenStatement);
    groupSites.push(
      buildIfBranchTransitionSite(context, current, match, groupLine, thenStatements),
    );
    hasPotentialIntent =
      hasPotentialIntent ||
      hasPotentialTransitionIntent(context, thenStatements, match.domainInfo);

    if (!current.elseStatement) {
      current = undefined;
      continue;
    }

    if (ts.isIfStatement(current.elseStatement)) {
      current = current.elseStatement;
      continue;
    }

    if (domainInfo) {
      hasPotentialIntent = appendIfElseTransitionSites(
        context,
        current,
        domainInfo,
        groupLine,
        matchedVariants,
        groupSites,
        hasPotentialIntent,
      );
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

function matchesIfTransitionGroup(
  domainInfo: ClosedDomainInfo | null,
  subjectPath: string | null,
  match: {
    domainInfo: ClosedDomainInfo;
    subjectPath: string;
    sourceVariant: string;
  },
): boolean {
  if (!domainInfo) {
    return true;
  }

  return (
    subjectPath === match.subjectPath &&
    domainInfo.domainSymbolName === match.domainInfo.domainSymbolName
  );
}

function initializeIfTransitionGroup(match: {
  domainInfo: ClosedDomainInfo;
  subjectPath: string;
}): { domainInfo: ClosedDomainInfo; subjectPath: string } {
  return {
    domainInfo: match.domainInfo,
    subjectPath: match.subjectPath,
  };
}

function buildIfBranchTransitionSite(
  context: FileAnalysisContext,
  current: ts.IfStatement,
  match: {
    domainInfo: ClosedDomainInfo;
    sourceVariant: string;
  },
  groupLine: number,
  thenStatements: readonly ts.Statement[],
): TransitionSite {
  return {
    path: context.relativePath,
    domain_symbol_name: match.domainInfo.domainSymbolName,
    group_id: `${context.relativePath}:${groupLine}:${match.domainInfo.domainSymbolName}`,
    transition_kind: TransitionKind.IfBranch,
    source_variant: match.sourceVariant,
    target_variants: collectTransitionTargetVariants(
      thenStatements,
      match.domainInfo.variants,
    ),
    line: lineOfNode(context.sourceFile, current.expression),
  };
}

function appendIfElseTransitionSites(
  context: FileAnalysisContext,
  current: ts.IfStatement,
  domainInfo: ClosedDomainInfo,
  groupLine: number,
  matchedVariants: Set<string>,
  groupSites: TransitionSite[],
  hasPotentialIntent: boolean,
): boolean {
  const elseStatements = statementsOf(current.elseStatement!);
  const remainingVariants = domainInfo.variants.filter(
    (variant) => !matchedVariants.has(variant),
  );
  const elseTargets = collectTransitionTargetVariants(elseStatements, domainInfo.variants);
  const nextHasPotentialIntent =
    hasPotentialIntent || hasPotentialTransitionIntent(context, elseStatements, domainInfo);

  for (const sourceVariant of remainingVariants) {
    groupSites.push({
      path: context.relativePath,
      domain_symbol_name: domainInfo.domainSymbolName,
      group_id: `${context.relativePath}:${groupLine}:${domainInfo.domainSymbolName}`,
      transition_kind: TransitionKind.IfElse,
      source_variant: sourceVariant,
      target_variants: elseTargets,
      line: lineOfNode(context.sourceFile, current.elseStatement!),
    });
  }

  return nextHasPotentialIntent;
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
