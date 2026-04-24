import ts from "typescript";

import type { ClosedDomainInfo, FileAnalysisContext } from "./analysis-types.js";
import type {
  ExhaustivenessFallbackKind as ExhaustivenessFallbackKindType,
  ExhaustivenessProofKind as ExhaustivenessProofKindType,
  ExhaustivenessSiteSemanticRole as ExhaustivenessSiteSemanticRoleType,
} from "./types.js";
import {
  ExhaustivenessFallbackKind,
  ExhaustivenessProofKind,
  ExhaustivenessSiteSemanticRole,
  ExhaustivenessSiteKind,
} from "./types.js";
import {
  expressionPath,
  isTopLevelDeclaration,
  lineOfNode,
  literalExpressionText,
  objectLiteralKeys,
  relativePath,
  unwrapObjectLiteralExpression,
} from "./analysis-utils.js";

export function collectClosedDomain(context: FileAnalysisContext, node: ts.Node): void {
  if (ts.isTypeAliasDeclaration(node) && isTopLevelDeclaration(node)) {
    const variants = closedDomainVariantsFromTypeNode(context, node.type);
    if (variants.length > 1) {
      context.closedDomains.push({
        path: context.relativePath,
        symbol_name: node.name.text,
        variants,
        line: lineOfNode(context.sourceFile, node.name),
        defining_file: context.relativePath,
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
        defining_file: context.relativePath,
      });
    }
  }
}

export function collectClosedDomainSite(context: FileAnalysisContext, node: ts.Node): void {
  if (ts.isSwitchStatement(node)) {
    collectSwitchExhaustivenessSite(context, node);
    return;
  }

  if (
    ts.isIfStatement(node) &&
    !(ts.isIfStatement(node.parent) && node.parent.elseStatement === node)
  ) {
    collectIfElseExhaustivenessSite(context, node);
    return;
  }

  if (ts.isConditionalExpression(node)) {
    collectConditionalExhaustivenessSite(context, node);
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
      defining_file: recordInfo.definingFile,
      site_kind: ExhaustivenessSiteKind.Record,
      proof_kind: ExhaustivenessProofKind.Record,
      covered_variants: objectLiteralKeys(initializerObjectLiteral),
      line: lineOfNode(context.sourceFile, node.name),
      fallback_kind: ExhaustivenessFallbackKind.None,
      site_expression: node.name.getText(context.sourceFile),
      site_semantic_role: semanticRoleForSite(node),
      site_confidence: 1,
    });
    return;
  }

  if (
    ts.isVariableDeclaration(node) &&
    !node.type &&
    initializerObjectLiteral &&
    !hasSatisfiesExpressionWrapper(node.initializer)
  ) {
    const keys = objectLiteralKeys(initializerObjectLiteral);
    const inferredInfo = closedDomainInfoForObjectKeys(
      context,
      keys,
      node.name.getText(context.sourceFile),
    );
    if (!inferredInfo) {
      return;
    }

    context.closedDomainSites.push({
      path: context.relativePath,
      domain_symbol_name: inferredInfo.domainInfo.domainSymbolName,
      defining_file: inferredInfo.domainInfo.definingFile,
      site_kind: ExhaustivenessSiteKind.Record,
      proof_kind: ExhaustivenessProofKind.Record,
      covered_variants: keys.filter((key) => inferredInfo.domainInfo.variants.includes(key)),
      line: lineOfNode(context.sourceFile, node.name),
      fallback_kind: ExhaustivenessFallbackKind.None,
      site_expression: node.name.getText(context.sourceFile),
      site_semantic_role: semanticRoleForSite(node),
      site_confidence: inferredInfo.confidence,
    });
    return;
  }

  if (ts.isVariableDeclaration(node) && node.initializer) {
    const mapSite = mapConstructorDomainSite(
      context,
      node.initializer,
      node.name.getText(context.sourceFile),
    );
    if (mapSite) {
      context.closedDomainSites.push({
        path: context.relativePath,
        domain_symbol_name: mapSite.domainInfo.domainSymbolName,
        defining_file: mapSite.domainInfo.definingFile,
        site_kind: ExhaustivenessSiteKind.Record,
        proof_kind: ExhaustivenessProofKind.Record,
        covered_variants: mapSite.keys.filter((key) =>
          mapSite.domainInfo.variants.includes(key),
        ),
        line: lineOfNode(context.sourceFile, node.name),
        fallback_kind: ExhaustivenessFallbackKind.None,
        site_expression: node.name.getText(context.sourceFile),
        site_semantic_role: semanticRoleForSite(node),
        site_confidence: mapSite.confidence,
      });
      return;
    }
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
      defining_file: recordInfo.definingFile,
      site_kind: ExhaustivenessSiteKind.Satisfies,
      proof_kind: ExhaustivenessProofKind.Satisfies,
      covered_variants: objectLiteralKeys(objectLiteral),
      line: lineOfNode(context.sourceFile, objectLiteral),
      fallback_kind: ExhaustivenessFallbackKind.None,
      site_expression: objectLiteral.getText(context.sourceFile).slice(0, 80),
      site_semantic_role: semanticRoleForSite(node),
      site_confidence: 1,
    });
  }
}

function hasSatisfiesExpressionWrapper(expression: ts.Expression | undefined): boolean {
  let current = expression;

  while (current) {
    if (ts.isSatisfiesExpression(current)) {
      return true;
    }
    if (
      ts.isParenthesizedExpression(current) ||
      ts.isAsExpression(current) ||
      ts.isTypeAssertionExpression(current) ||
      ts.isNonNullExpression(current)
    ) {
      current = current.expression;
      continue;
    }

    return false;
  }

  return false;
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
  let proofKind: ExhaustivenessProofKindType = ExhaustivenessProofKind.Switch;
  let fallbackKind: ExhaustivenessFallbackKindType = ExhaustivenessFallbackKind.None;
  for (const clause of node.caseBlock.clauses) {
    if (ts.isDefaultClause(clause)) {
      if (clause.statements.some((statement) => containsAssertNever(statement))) {
        proofKind = ExhaustivenessProofKind.AssertNever;
      }
      fallbackKind = classifyFallbackStatements(
        clause.statements,
        node.expression,
        domainInfo.variants,
      );
      continue;
    }

    const variant = literalExpressionText(clause.expression);
    if (variant) {
      coveredVariants.push(variant);
    }
  }

  if (proofKind !== ExhaustivenessProofKind.AssertNever && switchHasTrailingAssertNever(node)) {
    proofKind = ExhaustivenessProofKind.AssertNever;
    fallbackKind = ExhaustivenessFallbackKind.AssertThrow;
  }

  context.closedDomainSites.push({
    path: context.relativePath,
    domain_symbol_name: domainInfo.domainSymbolName,
    defining_file: domainInfo.definingFile,
    site_kind: ExhaustivenessSiteKind.Switch,
    proof_kind: proofKind,
    covered_variants: coveredVariants,
    line: lineOfNode(context.sourceFile, node.expression),
    fallback_kind: fallbackKind,
    site_expression: expressionPath(node.expression) ?? node.expression.getText(context.sourceFile),
    site_semantic_role: semanticRoleForSite(node),
    site_confidence: 1,
  });
}

function collectConditionalExhaustivenessSite(
  context: FileAnalysisContext,
  node: ts.ConditionalExpression,
): void {
  const match = ifConditionVariant(context, node.condition, null, null);
  if (!match || match.domainInfo.variants.length <= 1) {
    return;
  }

  const role = semanticRoleForSite(node);
  if (role === ExhaustivenessSiteSemanticRole.Unknown) {
    return;
  }

  context.closedDomainSites.push({
    path: context.relativePath,
    domain_symbol_name: match.domainInfo.domainSymbolName,
    defining_file: match.domainInfo.definingFile,
    site_kind: ExhaustivenessSiteKind.IfElse,
    proof_kind: ExhaustivenessProofKind.IfElse,
    covered_variants: [match.sourceVariant],
    line: lineOfNode(context.sourceFile, node.condition),
    fallback_kind: classifyFallbackExpression(
      node.whenFalse,
      null,
      match.domainInfo.variants,
    ),
    site_expression: match.subjectPath,
    site_semantic_role: role,
    site_confidence: 0.76,
  });
}

function collectIfElseExhaustivenessSite(
  context: FileAnalysisContext,
  node: ts.IfStatement,
): void {
  const coveredVariants: string[] = [];
  let current: ts.IfStatement | undefined = node;
  let domainInfo: ClosedDomainInfo | null = null;
  let subjectPath: string | null = null;
  let finalElse: ts.Statement | null = null;
  const groupLine = lineOfNode(context.sourceFile, node.expression);

  while (current) {
    const match = ifConditionVariant(context, current.expression, subjectPath, domainInfo);
    if (!match) {
      return;
    }

    if (domainInfo && domainInfo.domainSymbolName !== match.domainInfo.domainSymbolName) {
      return;
    }
    if (subjectPath && subjectPath !== match.subjectPath) {
      return;
    }
    if (!domainInfo) {
      domainInfo = match.domainInfo;
      subjectPath = match.subjectPath;
    }

    coveredVariants.push(match.sourceVariant);

    if (!current.elseStatement) {
      current = undefined;
      continue;
    }

    if (ts.isIfStatement(current.elseStatement)) {
      current = current.elseStatement;
      continue;
    }

    finalElse = current.elseStatement;
    current = undefined;
  }

  if (!domainInfo || domainInfo.variants.length <= 1 || coveredVariants.length === 0) {
    return;
  }

  context.closedDomainSites.push({
    path: context.relativePath,
    domain_symbol_name: domainInfo.domainSymbolName,
    defining_file: domainInfo.definingFile,
    site_kind: ExhaustivenessSiteKind.IfElse,
    proof_kind: ExhaustivenessProofKind.IfElse,
    covered_variants: [...new Set(coveredVariants)],
    line: groupLine,
    fallback_kind: finalElse
      ? classifyFallbackStatements(statementsOf(finalElse), null, domainInfo.variants)
      : ExhaustivenessFallbackKind.None,
    site_expression: subjectPath,
    site_semantic_role: semanticRoleForSite(node),
    site_confidence: 0.86,
  });
}

function recordDomainInfoFromTypeNode(
  context: FileAnalysisContext,
  typeNode: ts.TypeNode,
): { domainSymbolName: string; definingFile: string | null } | null {
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
    definingFile: domainInfo.definingFile,
  };
}

function closedDomainInfoForObjectKeys(
  context: FileAnalysisContext,
  keys: readonly string[],
  siteName: string,
): { domainInfo: ClosedDomainInfo; confidence: number } | null {
  const uniqueKeys = [...new Set(keys)];
  if (uniqueKeys.length < 2) {
    return null;
  }

  const candidates = visibleClosedDomainInfos(context)
    .filter((domainInfo) =>
      uniqueKeys.every((key) => domainInfo.variants.includes(key)),
    )
    .map((domainInfo) => ({
      domainInfo,
      exact: domainInfo.variants.length === uniqueKeys.length,
      nameHint: siteNameMatchesDomain(siteName, domainInfo.domainSymbolName),
    }));

  if (candidates.length === 0) {
    return null;
  }

  const exactMatches = candidates.filter((candidate) => candidate.exact);
  if (exactMatches.length === 1) {
    return { domainInfo: exactMatches[0].domainInfo, confidence: 0.94 };
  }

  const hintedMatches = candidates.filter((candidate) => candidate.nameHint);
  if (hintedMatches.length === 1) {
    return {
      domainInfo: hintedMatches[0].domainInfo,
      confidence: hintedMatches[0].exact ? 0.92 : 0.78,
    };
  }

  if (candidates.length === 1) {
    return {
      domainInfo: candidates[0].domainInfo,
      confidence: candidates[0].exact ? 0.9 : 0.72,
    };
  }

  return null;
}

function mapConstructorDomainSite(
  context: FileAnalysisContext,
  expression: ts.Expression,
  siteName: string,
): { domainInfo: ClosedDomainInfo; keys: string[]; confidence: number } | null {
  const current = unwrapSemanticExpression(expression);
  if (!ts.isNewExpression(current) || current.expression.getText(context.sourceFile) !== "Map") {
    return null;
  }

  const keys = mapConstructorLiteralKeys(current);
  if (keys.length < 2) {
    return null;
  }

  const typeNode = current.typeArguments?.[0] ?? null;
  if (typeNode) {
    const domainInfo = closedDomainInfoForTypeNode(context, typeNode);
    if (domainInfo && domainInfo.variants.length > 1) {
      return { domainInfo, keys, confidence: 0.96 };
    }
  }

  const inferredInfo = closedDomainInfoForObjectKeys(context, keys, siteName);
  if (!inferredInfo) {
    return null;
  }

  return {
    domainInfo: inferredInfo.domainInfo,
    keys,
    confidence: Math.min(inferredInfo.confidence, 0.82),
  };
}

function mapConstructorLiteralKeys(node: ts.NewExpression): string[] {
  const entries = mapConstructorEntries(node);
  if (!entries) {
    return [];
  }

  const keys: string[] = [];
  for (const entry of entries.elements) {
    const current = unwrapSemanticExpression(entry);
    if (!ts.isArrayLiteralExpression(current) || current.elements.length < 1) {
      continue;
    }

    const keyExpression = current.elements[0];
    if (!ts.isExpression(keyExpression)) {
      continue;
    }

    const key = literalExpressionText(unwrapSemanticExpression(keyExpression));
    if (key) {
      keys.push(key);
    }
  }

  return keys;
}

function mapConstructorEntries(node: ts.NewExpression): ts.ArrayLiteralExpression | null {
  const firstArgument = node.arguments?.[0];
  if (!firstArgument) {
    return null;
  }

  const current = unwrapSemanticExpression(firstArgument);
  return ts.isArrayLiteralExpression(current) ? current : null;
}

function visibleClosedDomainInfos(context: FileAnalysisContext): ClosedDomainInfo[] {
  const byDomain = new Map<string, ClosedDomainInfo>();

  for (const domain of context.closedDomains) {
    byDomain.set(`${domain.defining_file ?? domain.path}:${domain.symbol_name}`, {
      domainSymbolName: domain.symbol_name,
      definingFile: domain.defining_file,
      variants: domain.variants,
    });
  }

  const symbols = context.checker.getSymbolsInScope(
    context.sourceFile,
    ts.SymbolFlags.Type | ts.SymbolFlags.Enum | ts.SymbolFlags.Alias,
  );
  for (const symbol of symbols) {
    const domainInfo = closedDomainInfoFromVisibleSymbol(context, symbol);
    if (!domainInfo) {
      continue;
    }
    byDomain.set(`${domainInfo.definingFile ?? ""}:${domainInfo.domainSymbolName}`, domainInfo);
  }

  return [...byDomain.values()];
}

function closedDomainInfoFromVisibleSymbol(
  context: FileAnalysisContext,
  symbol: ts.Symbol,
): ClosedDomainInfo | null {
  const resolvedSymbol =
    symbol.flags & ts.SymbolFlags.Alias
      ? context.checker.getAliasedSymbol(symbol)
      : symbol;
  const declaration = resolvedSymbol.declarations?.find(
    (node) => ts.isTypeAliasDeclaration(node) || ts.isEnumDeclaration(node),
  );
  if (!declaration) {
    return null;
  }

  const type = context.checker.getDeclaredTypeOfSymbol(resolvedSymbol);
  return closedDomainInfoFromType(context, type);
}

function siteNameMatchesDomain(siteName: string, domainSymbolName: string): boolean {
  const normalizedSiteName = normalizeSemanticName(siteName);
  return domainNameStems(domainSymbolName).some((stem) =>
    normalizedSiteName.includes(stem),
  );
}

function domainNameStems(domainSymbolName: string): string[] {
  const normalized = normalizeSemanticName(domainSymbolName);
  const stems = new Set<string>([normalized]);
  for (const suffix of ["kind", "type", "status", "state", "mode", "variant"]) {
    if (normalized.endsWith(suffix) && normalized.length > suffix.length) {
      stems.add(normalized.slice(0, -suffix.length));
    }
  }
  return [...stems].filter((stem) => stem.length > 1);
}

function normalizeSemanticName(value: string): string {
  return value.replace(/[^a-zA-Z0-9]/g, "").toLowerCase();
}

export function closedDomainInfoForExpression(
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

export function closedDomainInfoForTypeNode(
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
        definingFile: definingFileFromType(context, type),
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
    definingFile: definingFileFromType(context, type),
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

function definingFileFromType(context: FileAnalysisContext, type: ts.Type): string | null {
  const aliasSymbol = type.aliasSymbol;
  const aliasDefiningFile = definingFileFromSymbol(context, aliasSymbol);
  if (aliasDefiningFile) {
    return aliasDefiningFile;
  }

  const typeSymbol = type.getSymbol();
  if (typeSymbol && typeSymbol.getName() !== "__type") {
    return definingFileFromSymbol(context, typeSymbol);
  }

  return null;
}

function definingFileFromSymbol(
  context: FileAnalysisContext,
  symbol: ts.Symbol | undefined,
): string | null {
  const declaration = symbol?.declarations?.find((node) => !ts.isTypeParameterDeclaration(node));
  if (!declaration) {
    return null;
  }

  return relativePath(context.rootPath, declaration.getSourceFile().fileName);
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

type IfConditionVariantMatch = {
  domainInfo: ClosedDomainInfo;
  subjectPath: string;
  sourceVariant: string;
};

function ifConditionVariant(
  context: FileAnalysisContext,
  expression: ts.Expression,
  expectedSubjectPath: string | null,
  expectedDomainInfo: ClosedDomainInfo | null,
): IfConditionVariantMatch | null {
  const current = unwrapSemanticExpression(expression);
  if (
    !ts.isBinaryExpression(current) ||
    !isVariantComparisonOperator(current.operatorToken.kind)
  ) {
    return null;
  }

  const leftPath = expressionPath(current.left);
  const rightPath = expressionPath(current.right);

  return (
    conditionVariantForSide(
      context,
      leftPath,
      current.left,
      current.right,
      current.operatorToken.kind,
      expectedSubjectPath,
      expectedDomainInfo,
    ) ??
    conditionVariantForSide(
      context,
      rightPath,
      current.right,
      current.left,
      current.operatorToken.kind,
      expectedSubjectPath,
      expectedDomainInfo,
    )
  );
}

function conditionVariantForSide(
  context: FileAnalysisContext,
  subjectPath: string | null,
  subjectExpression: ts.Expression,
  comparedExpression: ts.Expression,
  operatorKind: ts.SyntaxKind,
  expectedSubjectPath: string | null,
  expectedDomainInfo: ClosedDomainInfo | null,
): IfConditionVariantMatch | null {
  if (!subjectPath) {
    return null;
  }

  let domainInfo: ClosedDomainInfo | null = null;
  if (expectedSubjectPath === subjectPath && expectedDomainInfo) {
    domainInfo = expectedDomainInfo;
  } else {
    domainInfo = closedDomainInfoForExpression(context, subjectExpression);
  }
  if (!domainInfo) {
    return null;
  }

  const sourceVariant = sourceVariantForCondition(
    domainInfo.variants,
    literalExpressionText(comparedExpression),
    operatorKind,
  );
  if (!sourceVariant) {
    return null;
  }

  return {
    domainInfo,
    subjectPath,
    sourceVariant,
  };
}

function isVariantComparisonOperator(kind: ts.SyntaxKind): boolean {
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
    if (variants.includes(comparedVariant)) {
      return comparedVariant;
    }
    return null;
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

function classifyFallbackStatements(
  statements: readonly ts.Statement[],
  subjectExpression: ts.Expression | null,
  domainVariants: readonly string[],
): ExhaustivenessFallbackKindType {
  let sawStatement = false;

  for (const statement of statements) {
    sawStatement = true;
    if (containsAssertNever(statement) || ts.isThrowStatement(statement)) {
      return ExhaustivenessFallbackKind.AssertThrow;
    }

    if (ts.isReturnStatement(statement)) {
      return classifyFallbackExpression(statement.expression, subjectExpression, domainVariants);
    }

    if (ts.isExpressionStatement(statement)) {
      const expressionKind = classifyFallbackExpression(
        statement.expression,
        subjectExpression,
        domainVariants,
      );
      if (expressionKind !== ExhaustivenessFallbackKind.Other) {
        return expressionKind;
      }
    }
  }

  return sawStatement ? ExhaustivenessFallbackKind.Other : ExhaustivenessFallbackKind.Undefined;
}

function classifyFallbackExpression(
  expression: ts.Expression | undefined,
  subjectExpression: ts.Expression | null,
  domainVariants: readonly string[],
): ExhaustivenessFallbackKindType {
  if (!expression) {
    return ExhaustivenessFallbackKind.Undefined;
  }

  const current = unwrapSemanticExpression(expression);
  if (current.kind === ts.SyntaxKind.NullKeyword) {
    return ExhaustivenessFallbackKind.Null;
  }

  if (current.kind === ts.SyntaxKind.UndefinedKeyword) {
    return ExhaustivenessFallbackKind.Undefined;
  }

  if (ts.isIdentifier(current) && current.text === "undefined") {
    return ExhaustivenessFallbackKind.Undefined;
  }

  if (ts.isStringLiteral(current) || ts.isNoSubstitutionTemplateLiteral(current)) {
    return domainVariants.includes(current.text)
      ? ExhaustivenessFallbackKind.Other
      : ExhaustivenessFallbackKind.GenericString;
  }

  if (ts.isArrayLiteralExpression(current) && current.elements.length === 0) {
    return ExhaustivenessFallbackKind.EmptyArray;
  }

  if (ts.isObjectLiteralExpression(current) && current.properties.length === 0) {
    return ExhaustivenessFallbackKind.EmptyObject;
  }

  if (subjectExpression && expressionMatchesSubject(current, subjectExpression)) {
    return ExhaustivenessFallbackKind.IdentityTransform;
  }

  if (ts.isCallExpression(current) && isAssertThrowCall(current)) {
    return ExhaustivenessFallbackKind.AssertThrow;
  }

  return ExhaustivenessFallbackKind.Other;
}

function expressionMatchesSubject(
  expression: ts.Expression,
  subjectExpression: ts.Expression,
): boolean {
  if (expression.getText() === subjectExpression.getText()) {
    return true;
  }

  const discriminantInfo = discriminantAccessInfo(subjectExpression);
  if (!discriminantInfo) {
    return false;
  }

  return expression.getText() === discriminantInfo.baseExpression.getText();
}

function isAssertThrowCall(node: ts.CallExpression): boolean {
  const expressionText = node.expression.getText();
  return /(^|\.)(assertNever|assertUnreachable|unreachable|invariant|fail)$/.test(
    expressionText,
  );
}

function unwrapSemanticExpression(expression: ts.Expression): ts.Expression {
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

function semanticRoleForSite(node: ts.Node): ExhaustivenessSiteSemanticRoleType {
  const text = normalizeSemanticName(siteContextName(node));

  if (/(label|title|text|message|description|display|caption|name)/.test(text)) {
    return ExhaustivenessSiteSemanticRole.Label;
  }

  if (/(href|url|uri|link|target|route|path|destination)/.test(text)) {
    return ExhaustivenessSiteSemanticRole.Target;
  }

  if (/(status|state|lifecycle|phase)/.test(text)) {
    return ExhaustivenessSiteSemanticRole.Status;
  }

  if (/(render|view|component|jsx|template|screen)/.test(text) || siteContainsJsx(node)) {
    return ExhaustivenessSiteSemanticRole.Render;
  }

  if (/(handler|callback|onclick|onsubmit|onchange|listener)/.test(text)) {
    return ExhaustivenessSiteSemanticRole.Handler;
  }

  if (/(policy|permission|auth|allow|rule|guard)/.test(text)) {
    return ExhaustivenessSiteSemanticRole.Policy;
  }

  if (/(serialize|deserialize|json|dto|schema|payload|encode|decode)/.test(text)) {
    return ExhaustivenessSiteSemanticRole.Serialization;
  }

  if (/(transform|transition|rewrite|convert|map|mapper)/.test(text)) {
    return ExhaustivenessSiteSemanticRole.Transform;
  }

  return ExhaustivenessSiteSemanticRole.Unknown;
}

function siteContextName(node: ts.Node): string {
  let current: ts.Node | undefined = node;
  const names: string[] = [];

  while (current) {
    if (
      (ts.isFunctionDeclaration(current) ||
        ts.isFunctionExpression(current) ||
        ts.isClassDeclaration(current) ||
        ts.isMethodDeclaration(current)) &&
      current.name
    ) {
      names.push(current.name.getText());
    } else if (ts.isVariableDeclaration(current)) {
      names.push(current.name.getText());
    } else if (ts.isPropertyAssignment(current)) {
      names.push(current.name.getText());
    }

    current = current.parent;
  }

  return names.join(" ");
}

function siteContainsJsx(node: ts.Node): boolean {
  let found = false;

  function visit(child: ts.Node): void {
    if (found) {
      return;
    }
    if (
      ts.isJsxElement(child) ||
      ts.isJsxSelfClosingElement(child) ||
      ts.isJsxFragment(child)
    ) {
      found = true;
      return;
    }
    ts.forEachChild(child, visit);
  }

  visit(node);
  return found;
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
