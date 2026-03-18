import path from "node:path";
import ts from "typescript";
const PROTOCOL_VERSION = "0.1.0";
function isObject(value) {
    return typeof value === "object" && value !== null;
}
function isStringArray(value) {
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
function toRequest(value) {
    if (!isObject(value)) {
        return null;
    }
    return {
        jsonrpc: typeof value.jsonrpc === "string" ? value.jsonrpc : undefined,
        id: value.id,
        method: typeof value.method === "string" ? value.method : undefined,
        params: value.params,
    };
}
function toProjectModel(value) {
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
        primary_language: typeof value.primary_language === "string" ? value.primary_language : null,
        fingerprint: value.fingerprint,
    };
}
function normalizePath(value) {
    return value.replaceAll("\\", "/");
}
function relativePath(rootPath, filePath) {
    const relative = path.relative(rootPath, filePath);
    return normalizePath(relative.length > 0 ? relative : path.basename(filePath));
}
function lineOfNode(sourceFile, node) {
    return ts.getLineAndCharacterOfPosition(sourceFile, node.getStart(sourceFile)).line + 1;
}
function symbolId(relativeFilePath, name, line) {
    return `${relativeFilePath}::${name}:${line}`;
}
function writeMessage(message) {
    const body = JSON.stringify(message);
    const header = `Content-Length: ${Buffer.byteLength(body, "utf8")}\r\n\r\n`;
    process.stdout.write(header);
    process.stdout.write(body);
}
function respond(id, result, error) {
    const response = {
        jsonrpc: "2.0",
        id,
    };
    if (error) {
        response.error = error;
    }
    else {
        response.result = result ?? null;
    }
    writeMessage(response);
}
function errorResponse(id, code, message, data) {
    respond(id, undefined, { code, message, data });
}
function handleRequest(request) {
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
        }
        catch (error) {
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
function analyzeProject(project) {
    const rootPath = path.resolve(project.root);
    const fileFacts = [];
    const symbolFacts = [];
    const readFacts = [];
    const writeFacts = [];
    const closedDomains = [];
    const closedDomainSites = [];
    const seenFiles = new Set();
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
        }
    }
    return {
        project,
        analyzed_files: fileFacts.length,
        capabilities: ["Symbols", "Reads", "Writes", "ClosedDomains", "ClosedDomainSites"],
        files: fileFacts,
        symbols: symbolFacts,
        reads: readFacts,
        writes: writeFacts,
        closed_domains: closedDomains,
        closed_domain_sites: closedDomainSites,
    };
}
function createProgramFromTsconfig(tsconfigPath) {
    const configFile = ts.readConfigFile(tsconfigPath, ts.sys.readFile);
    if (configFile.error) {
        throw new Error(formatDiagnostic(configFile.error));
    }
    const parsed = ts.parseJsonConfigFileContent(configFile.config, ts.sys, path.dirname(tsconfigPath));
    if (parsed.errors.length > 0) {
        throw new Error(formatDiagnostics(parsed.errors));
    }
    return ts.createProgram({
        rootNames: parsed.fileNames,
        options: parsed.options,
    });
}
function shouldSkipSourceFile(sourceFile, rootPath, seenFiles) {
    if (sourceFile.isDeclarationFile) {
        return true;
    }
    const filePath = path.resolve(sourceFile.fileName);
    if (!normalizePath(filePath).startsWith(normalizePath(rootPath))) {
        return true;
    }
    return seenFiles.has(sourceFile.fileName);
}
function analyzeSourceFile(rootPath, sourceFile, checker) {
    const context = {
        rootPath,
        relativePath: relativePath(rootPath, sourceFile.fileName),
        sourceFile,
        checker,
        symbolFacts: [],
        readFacts: [],
        writeFacts: [],
        closedDomains: [],
        closedDomainSites: [],
    };
    function visit(node) {
        collectSymbolFact(context, node);
        collectReadFacts(context, node);
        collectWriteFacts(context, node);
        collectClosedDomain(context, node);
        collectClosedDomainSite(context, node);
        ts.forEachChild(node, visit);
    }
    ts.forEachChild(sourceFile, visit);
    return context;
}
function collectSymbolFact(context, node) {
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
    if (ts.isVariableDeclaration(node) &&
        ts.isIdentifier(node.name) &&
        isTopLevelVariableDeclaration(node)) {
        pushSymbolFact(context, node.name, "variable");
        const objectLiteral = node.initializer
            ? unwrapObjectLiteralExpression(node.initializer)
            : null;
        if (objectLiteral) {
            collectObjectPropertySymbolFacts(context, node.name.text, objectLiteral);
        }
    }
}
function pushSymbolFact(context, identifier, kind) {
    pushNamedSymbolFact(context, identifier.text, identifier, kind);
}
function pushNamedSymbolFact(context, name, node, kind) {
    const line = lineOfNode(context.sourceFile, node);
    context.symbolFacts.push({
        id: symbolId(context.relativePath, name, line),
        path: context.relativePath,
        name,
        kind,
        line,
    });
}
function collectObjectPropertySymbolFacts(context, prefix, objectLiteral) {
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
function collectReadFacts(context, node) {
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
function collectWriteFacts(context, node) {
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
    if ((ts.isPrefixUnaryExpression(node) || ts.isPostfixUnaryExpression(node)) &&
        isMutationOperator(node.operator)) {
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
function collectClosedDomain(context, node) {
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
        const variants = [];
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
function collectClosedDomainSite(context, node) {
    if (ts.isSwitchStatement(node)) {
        collectSwitchExhaustivenessSite(context, node);
        return;
    }
    const initializerObjectLiteral = ts.isVariableDeclaration(node) && node.initializer
        ? unwrapObjectLiteralExpression(node.initializer)
        : null;
    if (ts.isVariableDeclaration(node) &&
        node.type &&
        initializerObjectLiteral) {
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
function unwrapObjectLiteralExpression(expression) {
    let current = expression;
    while (ts.isParenthesizedExpression(current) ||
        ts.isAsExpression(current) ||
        ts.isTypeAssertionExpression(current) ||
        ts.isSatisfiesExpression(current)) {
        current = current.expression;
    }
    return ts.isObjectLiteralExpression(current) ? current : null;
}
function collectSwitchExhaustivenessSite(context, node) {
    const domainInfo = closedDomainInfoForExpression(context, node.expression);
    if (!domainInfo || domainInfo.variants.length <= 1) {
        return;
    }
    const coveredVariants = [];
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
function recordDomainInfoFromTypeNode(context, typeNode) {
    if (!ts.isTypeReferenceNode(typeNode)) {
        return null;
    }
    const typeName = typeNode.typeName.getText(context.sourceFile);
    if (typeName !== "Record" ||
        !typeNode.typeArguments ||
        typeNode.typeArguments.length < 1) {
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
function closedDomainInfoForExpression(context, expression) {
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
function closedDomainInfoForTypeNode(context, typeNode) {
    const type = context.checker.getTypeFromTypeNode(typeNode);
    return closedDomainInfoFromType(context, type);
}
function closedDomainInfoFromType(context, type, preferredDiscriminant) {
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
    const discriminatedInfo = discriminatedUnionInfoFromType(context, type, preferredDiscriminant);
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
function closedDomainVariantsFromTypeNode(context, typeNode) {
    const variants = literalUnionVariants(typeNode);
    if (variants.length > 1) {
        return variants;
    }
    const type = context.checker.getTypeFromTypeNode(typeNode);
    return discriminatedUnionInfoFromType(context, type)?.variants ?? [];
}
function domainSymbolNameFromType(type) {
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
function discriminantAccessInfo(expression) {
    if (ts.isPropertyAccessExpression(expression)) {
        return {
            baseExpression: expression.expression,
            propertyName: expression.name.text,
        };
    }
    if (ts.isElementAccessExpression(expression) &&
        expression.argumentExpression &&
        ts.isStringLiteral(expression.argumentExpression)) {
        return {
            baseExpression: expression.expression,
            propertyName: expression.argumentExpression.text,
        };
    }
    return null;
}
function discriminatedUnionInfoFromType(context, type, preferredDiscriminant) {
    if (!type.isUnion()) {
        return null;
    }
    const candidateNames = discriminantCandidateNames(type, preferredDiscriminant);
    for (const propertyName of candidateNames) {
        const variants = [];
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
function discriminantCandidateNames(type, preferredDiscriminant) {
    const commonNames = commonPropertyNames(type);
    const prioritized = [preferredDiscriminant, "kind", "type", "status", "state"].filter((value) => Boolean(value));
    const candidates = [];
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
function commonPropertyNames(type) {
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
function literalVariantsFromType(type) {
    if (type.isUnion()) {
        const variants = [];
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
function enumVariantsFromType(type) {
    const symbol = type.getSymbol();
    if (!symbol || !(symbol.flags & ts.SymbolFlags.Enum)) {
        return [];
    }
    const declaration = symbol.declarations?.find(ts.isEnumDeclaration);
    if (!declaration) {
        return [];
    }
    return declaration.members.map((member) => member.name.getText(declaration.getSourceFile()));
}
function literalVariantFromType(type) {
    if (type.flags & ts.TypeFlags.StringLiteral) {
        return type.value;
    }
    if (type.flags & ts.TypeFlags.NumberLiteral) {
        return String(type.value);
    }
    if (type.flags & ts.TypeFlags.BooleanLiteral) {
        return type.intrinsicName ?? null;
    }
    return null;
}
function objectLiteralKeys(node) {
    const keys = [];
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
function propertyNameText(name) {
    if (ts.isIdentifier(name) || ts.isStringLiteral(name) || ts.isNumericLiteral(name)) {
        return name.text;
    }
    return null;
}
function literalExpressionText(expression) {
    if (ts.isStringLiteral(expression) || ts.isNumericLiteral(expression)) {
        return expression.text;
    }
    if (expression.kind === ts.SyntaxKind.TrueKeyword ||
        expression.kind === ts.SyntaxKind.FalseKeyword) {
        return expression.getText();
    }
    return null;
}
function containsAssertNever(node) {
    let found = false;
    function visit(child) {
        if (found) {
            return;
        }
        if (ts.isCallExpression(child) &&
            ts.isIdentifier(child.expression) &&
            child.expression.text === "assertNever") {
            found = true;
            return;
        }
        ts.forEachChild(child, visit);
    }
    visit(node);
    return found;
}
function switchHasTrailingAssertNever(node) {
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
function switchAssertNeverSubjects(expression) {
    const subjects = new Set([expression.getText()]);
    const discriminantInfo = discriminantAccessInfo(expression);
    if (discriminantInfo) {
        subjects.add(discriminantInfo.baseExpression.getText());
    }
    return subjects;
}
function statementContainsAssertNeverForSubjects(node, subjects) {
    let found = false;
    function visit(child) {
        if (found) {
            return;
        }
        if (ts.isCallExpression(child) &&
            ts.isIdentifier(child.expression) &&
            child.expression.text === "assertNever") {
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
function isTopLevelDeclaration(node) {
    return node.parent !== undefined && ts.isSourceFile(node.parent);
}
function isTopLevelVariableDeclaration(node) {
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
function literalUnionVariants(typeNode) {
    if (!ts.isUnionTypeNode(typeNode)) {
        return [];
    }
    const variants = [];
    for (const member of typeNode.types) {
        const variant = literalTypeText(member);
        if (!variant) {
            return [];
        }
        variants.push(variant);
    }
    return variants;
}
function literalTypeText(typeNode) {
    if (!ts.isLiteralTypeNode(typeNode)) {
        return null;
    }
    if (ts.isStringLiteral(typeNode.literal) || ts.isNumericLiteral(typeNode.literal)) {
        return typeNode.literal.text;
    }
    if (typeNode.literal.kind === ts.SyntaxKind.TrueKeyword ||
        typeNode.literal.kind === ts.SyntaxKind.FalseKeyword) {
        return typeNode.literal.getText();
    }
    return null;
}
function expressionName(expression) {
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
function expressionPath(expression) {
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
    if (ts.isElementAccessExpression(expression) &&
        expression.argumentExpression &&
        (ts.isStringLiteral(expression.argumentExpression) ||
            ts.isNumericLiteral(expression.argumentExpression))) {
        const base = expressionPath(expression.expression);
        if (!base) {
            return null;
        }
        return `${base}.${expression.argumentExpression.text}`;
    }
    return null;
}
function isWriteTarget(node) {
    const parent = node.parent;
    if (!parent) {
        return false;
    }
    if (ts.isBinaryExpression(parent) && parent.left === node) {
        return isAssignmentOperator(parent.operatorToken.kind);
    }
    if ((ts.isPrefixUnaryExpression(parent) || ts.isPostfixUnaryExpression(parent)) &&
        parent.operand === node) {
        return isMutationOperator(parent.operator);
    }
    return false;
}
function setStoreTarget(argumentsList) {
    if (argumentsList.length === 0) {
        return null;
    }
    const segments = [];
    const selectorArguments = argumentsList.slice(0, argumentsList.length > 1 ? argumentsList.length - 1 : argumentsList.length);
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
function isAssignmentOperator(kind) {
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
function isMutationOperator(kind) {
    return kind === ts.SyntaxKind.PlusPlusToken || kind === ts.SyntaxKind.MinusMinusToken;
}
function formatDiagnostic(diagnostic) {
    return ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n");
}
function formatDiagnostics(diagnostics) {
    return diagnostics.map(formatDiagnostic).join("\n");
}
function parseHeaders(headerText) {
    const headers = new Map();
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
function main() {
    let buffer = Buffer.alloc(0);
    process.stdin.on("data", function handleData(chunk) {
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
            let parsed;
            try {
                parsed = JSON.parse(bodyText);
            }
            catch (error) {
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
    process.stdin.on("end", function handleEnd() {
        process.exit(0);
    });
}
main();
