import path from "node:path";
import ts from "typescript";

export function normalizePath(value: string): string {
  return value.replaceAll("\\", "/");
}

export function relativePath(rootPath: string, filePath: string): string {
  const relative = path.relative(rootPath, filePath);
  if (relative.length === 0) {
    return normalizePath(path.basename(filePath));
  }

  return normalizePath(relative);
}

export function lineOfNode(sourceFile: ts.SourceFile, node: ts.Node): number {
  return ts.getLineAndCharacterOfPosition(sourceFile, node.getStart(sourceFile)).line + 1;
}

export function symbolId(relativeFilePath: string, name: string, line: number): string {
  return `${relativeFilePath}::${name}:${line}`;
}

export function createProgramFromTsconfig(tsconfigPath: string): ts.Program {
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

export function shouldSkipSourceFile(
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

export function unwrapObjectLiteralExpression(
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

  if (!ts.isObjectLiteralExpression(current)) {
    return null;
  }

  return current;
}

export function expressionName(expression: ts.Expression): string | null {
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

export function expressionPath(expression: ts.Expression): string | null {
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

export function isAssignmentOperator(kind: ts.SyntaxKind): boolean {
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

export function isMutationOperator(kind: ts.SyntaxKind): boolean {
  return kind === ts.SyntaxKind.PlusPlusToken || kind === ts.SyntaxKind.MinusMinusToken;
}

export function propertyNameText(name: ts.PropertyName): string | null {
  if (ts.isIdentifier(name) || ts.isStringLiteral(name) || ts.isNumericLiteral(name)) {
    return name.text;
  }

  return null;
}

export function objectLiteralKeys(node: ts.ObjectLiteralExpression): string[] {
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

export function literalExpressionText(expression: ts.Expression): string | null {
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

export function isTopLevelDeclaration(node: ts.Node): boolean {
  return node.parent !== undefined && ts.isSourceFile(node.parent);
}

export function isTopLevelVariableDeclaration(node: ts.VariableDeclaration): boolean {
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

function formatDiagnostic(diagnostic: ts.Diagnostic): string {
  return ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n");
}

function formatDiagnostics(diagnostics: readonly ts.Diagnostic[]): string {
  return diagnostics.map(formatDiagnostic).join("\n");
}
