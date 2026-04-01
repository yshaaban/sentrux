import ts from "typescript";
import { ExhaustivenessProofKind, ExhaustivenessSiteKind, } from "./types.js";
import { isTopLevelDeclaration, lineOfNode, literalExpressionText, objectLiteralKeys, relativePath, unwrapObjectLiteralExpression, } from "./analysis-utils.js";
export function collectClosedDomain(context, node) {
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
                defining_file: context.relativePath,
            });
        }
    }
}
export function collectClosedDomainSite(context, node) {
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
            defining_file: recordInfo.definingFile,
            site_kind: ExhaustivenessSiteKind.Record,
            proof_kind: ExhaustivenessProofKind.Record,
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
            defining_file: recordInfo.definingFile,
            site_kind: ExhaustivenessSiteKind.Satisfies,
            proof_kind: ExhaustivenessProofKind.Satisfies,
            covered_variants: objectLiteralKeys(objectLiteral),
            line: lineOfNode(context.sourceFile, objectLiteral),
        });
    }
}
function collectSwitchExhaustivenessSite(context, node) {
    const domainInfo = closedDomainInfoForExpression(context, node.expression);
    if (!domainInfo || domainInfo.variants.length <= 1) {
        return;
    }
    const coveredVariants = [];
    let proofKind = ExhaustivenessProofKind.Switch;
    for (const clause of node.caseBlock.clauses) {
        if (ts.isDefaultClause(clause)) {
            if (clause.statements.some((statement) => containsAssertNever(statement))) {
                proofKind = ExhaustivenessProofKind.AssertNever;
            }
            continue;
        }
        const variant = literalExpressionText(clause.expression);
        if (variant) {
            coveredVariants.push(variant);
        }
    }
    if (proofKind !== ExhaustivenessProofKind.AssertNever && switchHasTrailingAssertNever(node)) {
        proofKind = ExhaustivenessProofKind.AssertNever;
    }
    context.closedDomainSites.push({
        path: context.relativePath,
        domain_symbol_name: domainInfo.domainSymbolName,
        defining_file: domainInfo.definingFile,
        site_kind: ExhaustivenessSiteKind.Switch,
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
        definingFile: domainInfo.definingFile,
    };
}
export function closedDomainInfoForExpression(context, expression) {
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
export function closedDomainInfoForTypeNode(context, typeNode) {
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
                definingFile: definingFileFromType(context, type),
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
        definingFile: definingFileFromType(context, type),
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
function definingFileFromType(context, type) {
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
function definingFileFromSymbol(context, symbol) {
    const declaration = symbol?.declarations?.find((node) => !ts.isTypeParameterDeclaration(node));
    if (!declaration) {
        return null;
    }
    return relativePath(context.rootPath, declaration.getSourceFile().fileName);
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
