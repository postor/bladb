#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import ts from "typescript";

const DEFAULT_EXTENSIONS = new Set([".ts", ".tsx", ".js", ".jsx", ".mts", ".cts", ".mjs", ".cjs"]);
const IGNORED_SEGMENTS = new Set([
  ".git",
  "node_modules",
  "dist",
  "build",
  "coverage",
  "target",
  ".next",
  ".turbo"
]);

function parseArgs(argv) {
  const options = {
    cwd: process.cwd(),
    format: "json",
    output: null,
    includeSuggestions: true
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--cwd") {
      options.cwd = path.resolve(argv[++index] ?? options.cwd);
      continue;
    }
    if (arg === "--format") {
      options.format = argv[++index] ?? "json";
      continue;
    }
    if (arg === "--output") {
      options.output = path.resolve(argv[++index] ?? "");
      continue;
    }
    if (arg === "--no-suggestions") {
      options.includeSuggestions = false;
      continue;
    }
    if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    }

    if (!arg.startsWith("--")) {
      options.cwd = path.resolve(arg);
      continue;
    }

    throw new Error(`Unknown argument: ${arg}`);
  }

  return options;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: bladb-extract [path] [--cwd <path>] [--format json] [--output <file>] [--no-suggestions]",
      "",
      "Scans a project for Bladb client calls and emits extracted operations plus policy hints."
    ].join("\n")
  );
}

function walkFiles(rootDir) {
  const results = [];
  const queue = [rootDir];

  while (queue.length > 0) {
    const current = queue.pop();
    const entries = fs.readdirSync(current, { withFileTypes: true });

    for (const entry of entries) {
      const fullPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        if (IGNORED_SEGMENTS.has(entry.name)) {
          continue;
        }
        queue.push(fullPath);
        continue;
      }

      if (DEFAULT_EXTENSIONS.has(path.extname(entry.name))) {
        results.push(fullPath);
      }
    }
  }

  return results;
}

function createSourceFile(filePath) {
  const sourceText = fs.readFileSync(filePath, "utf8");
  return ts.createSourceFile(
    filePath,
    sourceText,
    ts.ScriptTarget.Latest,
    true,
    scriptKindForFile(filePath)
  );
}

function scriptKindForFile(filePath) {
  const extension = path.extname(filePath);
  switch (extension) {
    case ".tsx":
      return ts.ScriptKind.TSX;
    case ".jsx":
      return ts.ScriptKind.JSX;
    case ".js":
    case ".mjs":
    case ".cjs":
      return ts.ScriptKind.JS;
    default:
      return ts.ScriptKind.TS;
  }
}

function extractProject(rootDir) {
  const files = walkFiles(rootDir);
  const operations = [];

  for (const filePath of files) {
    const sourceFile = createSourceFile(filePath);
    visitNode(sourceFile, sourceFile, operations, rootDir);
  }

  const suggestions = buildSuggestions(operations);

  return {
    root: rootDir,
    scannedFiles: files.length,
    operations,
    suggestedPolicies: suggestions
  };
}

function visitNode(node, sourceFile, operations, rootDir) {
  if (ts.isTaggedTemplateExpression(node)) {
    const sqlOperation = extractSqlOperation(node, sourceFile, rootDir);
    if (sqlOperation) {
      operations.push(sqlOperation);
    }
  } else if (ts.isCallExpression(node)) {
    const callOperation = extractCallOperation(node, sourceFile, rootDir);
    if (callOperation) {
      operations.push(callOperation);
    }
  }

  ts.forEachChild(node, (child) => visitNode(child, sourceFile, operations, rootDir));
}

function extractSqlOperation(node, sourceFile, rootDir) {
  const chain = parseExpressionChain(node.tag);
  if (!chain || chain.type !== "client" || chain.engine !== "sql") {
    return null;
  }

  const template = readTemplate(node.template);
  const statement = collapseTemplateText(template.parts);
  const action = classifySql(statement);
  const meta = serializeMetaLiteral(chain.meta);
  const location = locationOf(sourceFile, node, rootDir);

  return {
    type: "operation",
    engine: "sql",
    kind: action === "select" ? "query" : "command",
    action,
    file: location.file,
    line: location.line,
    column: location.column,
    resource: meta?.resource ?? null,
    policy: meta?.policy ?? null,
    params: meta?.params ?? {},
    statement,
    placeholders: template.expressions,
    target: inferSqlTargets(statement)
  };
}

function extractCallOperation(node, sourceFile, rootDir) {
  const chain = parseExpressionChain(node.expression);
  if (!chain || chain.type !== "client") {
    return null;
  }

  if (chain.engine === "mongo") {
    const collection = node.arguments[0] && chain.stage === "builder" ? readSimple(node.arguments[0]) : chain.collection;
    if (chain.stage === "builder") {
      return null;
    }

    const location = locationOf(sourceFile, node, rootDir);
    return {
      type: "operation",
      engine: "mongo",
      kind: chain.action === "insertOne" ? "command" : "query",
      action: chain.action,
      file: location.file,
      line: location.line,
      column: location.column,
      resource: serializeMetaLiteral(chain.meta)?.resource ?? collection ?? null,
      policy: serializeMetaLiteral(chain.meta)?.policy ?? null,
      params: serializeMetaLiteral(chain.meta)?.params ?? {},
      target: collection ? { collection } : null,
      args: node.arguments.map((arg) => summarizeExpression(arg))
    };
  }

  if (chain.engine && chain.action) {
    const meta = serializeMetaLiteral(chain.meta);
    const location = locationOf(sourceFile, node, rootDir);
    return {
      type: "operation",
      engine: chain.engine,
      kind: inferKind(chain.engine, chain.action),
      action: chain.action,
      file: location.file,
      line: location.line,
      column: location.column,
      resource: meta?.resource ?? null,
      policy: meta?.policy ?? null,
      params: meta?.params ?? {},
      target: inferCallTarget(chain, node.arguments),
      args: node.arguments.map((arg) => summarizeExpression(arg))
    };
  }

  return null;
}

function parseExpressionChain(expression) {
  if (ts.isPropertyAccessExpression(expression)) {
    const base = parseExpressionChain(expression.expression);
    if (!base) {
      return null;
    }

    if (base.type === "client" && expression.name.text === "sql") {
      return {
        type: "client",
        engine: "sql",
        meta: base.meta
      };
    }

    if (base.type === "mongo-builder" && ["find", "findOne", "insertOne"].includes(expression.name.text)) {
      return {
        type: "client",
        engine: "mongo",
        action: expression.name.text,
        collection: base.collection,
        meta: base.meta,
        stage: "operation"
      };
    }

    if (base.type === "client" && ["redis", "mqtt", "kafka", "mq"].includes(expression.name.text)) {
      return {
        type: "engine-root",
        engine: expression.name.text,
        meta: base.meta
      };
    }

    if (
      base.type === "engine-root" &&
      isEngineAction(base.engine, expression.name.text)
    ) {
      return {
        type: "client",
        engine: base.engine,
        action: expression.name.text,
        meta: base.meta
      };
    }

    return null;
  }

  if (ts.isCallExpression(expression)) {
    if (ts.isIdentifier(expression.expression) && expression.expression.text === "createClient") {
      return { type: "client", meta: null };
    }

    if (
      ts.isPropertyAccessExpression(expression.expression) &&
      expression.expression.name.text === "withMeta"
    ) {
      const base = parseExpressionChain(expression.expression.expression);
      if (!base || (base.type !== "client" && base.type !== "engine-root")) {
        return null;
      }

      const metaArg = expression.arguments[0];
      return {
        type: "client",
        meta: mergeMetaLiteral(base.meta, extractObjectLiteral(metaArg))
      };
    }

    if (
      ts.isPropertyAccessExpression(expression.expression) &&
      expression.expression.name.text === "mongo"
    ) {
      const base = parseExpressionChain(expression.expression.expression);
      if (!base || base.type !== "client") {
        return null;
      }

      return {
        type: "mongo-builder",
        collection: readSimple(expression.arguments[0]),
        meta: base.meta
      };
    }
  }

  if (ts.isIdentifier(expression) && expression.text === "db") {
    return { type: "client", meta: null };
  }

  return null;
}

function isEngineAction(engine, action) {
  const actions = {
    redis: new Set(["get", "set", "incrby", "decrby", "publish"]),
    mqtt: new Set(["publish"]),
    kafka: new Set(["produce"]),
    mq: new Set(["publish", "publishDelayed"])
  };

  return actions[engine]?.has(action) ?? false;
}

function readTemplate(template) {
  if (ts.isNoSubstitutionTemplateLiteral(template)) {
    return {
      parts: [template.text],
      expressions: []
    };
  }

  const parts = [template.head.text];
  const expressions = [];
  for (const span of template.templateSpans) {
    expressions.push(summarizeExpression(span.expression));
    parts.push(span.literal.text);
  }

  return { parts, expressions };
}

function collapseTemplateText(parts) {
  return parts
    .map((part, index) => (index === parts.length - 1 ? part : `${part}?`))
    .join("")
    .replace(/\s+/g, " ")
    .trim();
}

function classifySql(statement) {
  const verb = statement.split(/\s+/, 1)[0]?.toLowerCase();
  switch (verb) {
    case "insert":
    case "update":
    case "delete":
      return verb;
    default:
      return "select";
  }
}

function inferSqlTargets(statement) {
  const lowered = statement.toLowerCase();
  const tokens = lowered.split(/\s+/);
  if (tokens[0] === "select" || tokens[0] === "delete") {
    const fromIndex = tokens.indexOf("from");
    if (fromIndex >= 0 && tokens[fromIndex + 1]) {
      return { tables: [sanitizeToken(tokens[fromIndex + 1])] };
    }
  }
  if (tokens[0] === "insert") {
    const intoIndex = tokens.indexOf("into");
    if (intoIndex >= 0 && tokens[intoIndex + 1]) {
      return { tables: [sanitizeToken(tokens[intoIndex + 1])] };
    }
  }
  if (tokens[0] === "update" && tokens[1]) {
    return { tables: [sanitizeToken(tokens[1])] };
  }
  return null;
}

function sanitizeToken(value) {
  return value.replace(/[^a-zA-Z0-9_]/g, "");
}

function inferKind(engine, action) {
  if (engine === "redis") {
    return action === "get" ? "query" : action === "publish" ? "stream" : "command";
  }
  if (engine === "mqtt" || engine === "kafka") {
    return "stream";
  }
  if (engine === "mq") {
    return "queue";
  }
  return "query";
}

function inferCallTarget(chain, args) {
  if (chain.engine === "redis") {
    return {
      name: summarizeExpression(args[0])
    };
  }
  if (chain.engine === "mqtt" || chain.engine === "kafka") {
    return {
      topic: summarizeExpression(args[0])
    };
  }
  if (chain.engine === "mq") {
    return {
      queue: summarizeExpression(args[0])
    };
  }
  if (chain.engine === "mongo") {
    return chain.collection ? { collection: chain.collection } : null;
  }
  return null;
}

function extractObjectLiteral(node) {
  if (!node || !ts.isObjectLiteralExpression(node)) {
    return null;
  }

  const result = {};
  for (const property of node.properties) {
    if (!ts.isPropertyAssignment(property) || !ts.isIdentifier(property.name)) {
      continue;
    }
    result[property.name.text] = summarizeExpression(property.initializer);
  }
  return result;
}

function mergeMetaLiteral(base, patch) {
  if (!base && !patch) {
    return null;
  }
  return {
    ...(base ?? {}),
    ...(patch ?? {}),
    params: {
      ...(base?.params ?? {}),
      ...(patch?.params && typeof patch.params === "object" ? patch.params : {})
    }
  };
}

function serializeMetaLiteral(meta) {
  if (!meta) {
    return null;
  }
  return {
    resource: typeof meta.resource === "string" ? meta.resource : null,
    policy: typeof meta.policy === "string" ? meta.policy : null,
    params: meta.params && typeof meta.params === "object" ? meta.params : {}
  };
}

function readSimple(node) {
  if (!node) {
    return null;
  }
  if (ts.isStringLiteralLike(node)) {
    return node.text;
  }
  return null;
}

function summarizeExpression(node) {
  if (!node) {
    return null;
  }
  if (ts.isStringLiteralLike(node) || ts.isNumericLiteral(node)) {
    return node.text;
  }
  if (node.kind === ts.SyntaxKind.TrueKeyword) {
    return true;
  }
  if (node.kind === ts.SyntaxKind.FalseKeyword) {
    return false;
  }
  if (ts.isIdentifier(node)) {
    return node.text;
  }
  if (ts.isObjectLiteralExpression(node)) {
    const object = {};
    for (const property of node.properties) {
      if (!ts.isPropertyAssignment(property)) {
        continue;
      }
      const key = ts.isIdentifier(property.name) || ts.isStringLiteralLike(property.name)
        ? property.name.text
        : property.name.getText();
      object[key] = summarizeExpression(property.initializer);
    }
    return object;
  }
  if (ts.isArrayLiteralExpression(node)) {
    return node.elements.map((element) => summarizeExpression(element));
  }
  if (ts.isTaggedTemplateExpression(node)) {
    const template = readTemplate(node.template);
    return {
      template: template.parts,
      expressions: template.expressions
    };
  }
  if (ts.isTemplateExpression(node) || ts.isNoSubstitutionTemplateLiteral(node)) {
    const template = readTemplate(node);
    return {
      template: template.parts,
      expressions: template.expressions
    };
  }
  if (ts.isPropertyAccessExpression(node)) {
    return node.getText();
  }
  if (ts.isCallExpression(node)) {
    return node.getText();
  }
  return node.getText();
}

function locationOf(sourceFile, node, rootDir) {
  const position = sourceFile.getLineAndCharacterOfPosition(node.getStart());
  return {
    file: path.relative(rootDir, sourceFile.fileName).replaceAll("\\", "/"),
    line: position.line + 1,
    column: position.character + 1
  };
}

function buildSuggestions(operations) {
  const suggestions = [];
  const seen = new Set();

  for (const operation of operations) {
    const policyName =
      operation.policy ??
      [operation.engine, operation.action, deriveTargetName(operation)]
        .filter(Boolean)
        .join(".");
    if (seen.has(policyName)) {
      continue;
    }
    seen.add(policyName);

    const suggestion = {
      name: policyName,
      match: buildMatchSuggestion(operation),
      enforce: buildEnforceSuggestion(operation)
    };
    suggestions.push(suggestion);
  }

  return suggestions;
}

function buildMatchSuggestion(operation) {
  const match = {
    engine: operation.engine
  };

  if (operation.engine === "sql") {
    match.operation = operation.action;
    if (operation.target?.tables?.length) {
      match.tables = operation.target.tables;
    }
    return match;
  }

  if (operation.engine === "mongo") {
    match.collection = operation.target?.collection ?? null;
    match.action = operation.action;
    return match;
  }

  if (operation.engine === "redis") {
    match.command = operation.action;
    return match;
  }

  match.action = operation.action;
  return match;
}

function buildEnforceSuggestion(operation) {
  if (operation.engine === "sql") {
    return {
      where:
        operation.action === "select" || operation.action === "delete" || operation.action === "update"
          ? {
              uid: "{UID}",
              tenant_id: "{TENANT_ID}"
            }
          : undefined,
      fields:
        operation.action === "insert"
          ? {
              uid: "{UID}",
              tenant_id: "{TENANT_ID}"
            }
          : undefined
    };
  }

  if (operation.engine === "mongo") {
    return {
      query: {
        ownerUid: "{UID}",
        tenantId: "{TENANT_ID}"
      }
    };
  }

  if (operation.engine === "redis") {
    return {
      key: {
        template: summarizeTemplateSuggestion(operation.target?.name)
      }
    };
  }

  if (operation.engine === "mqtt" || operation.engine === "kafka") {
    return {
      topic: {
        template: summarizeTemplateSuggestion(operation.target?.topic)
      }
    };
  }

  if (operation.engine === "mq") {
    return {
      queue: operation.target?.queue ?? null
    };
  }

  return {};
}

function deriveTargetName(operation) {
  if (operation.resource) {
    return operation.resource;
  }
  if (operation.engine === "mongo") {
    return operation.target?.collection ?? "collection";
  }
  if (operation.engine === "sql") {
    return operation.target?.tables?.[0] ?? "statement";
  }
  if (operation.engine === "redis") {
    return deriveTemplateName(operation.target?.name, "key");
  }
  if (operation.engine === "mqtt" || operation.engine === "kafka") {
    return deriveTemplateName(operation.target?.topic, "topic");
  }
  if (operation.engine === "mq") {
    return operation.target?.queue ?? "queue";
  }
  return "operation";
}

function deriveTemplateName(target, fallback) {
  if (!target) {
    return fallback;
  }
  if (typeof target === "string") {
    return target
      .split(/[:/]/)
      .filter(Boolean)
      .slice(-2)
      .join(".") || fallback;
  }
  if (target.template) {
    const parts = target.template.map((part) => part.replace(/[^a-zA-Z0-9]+/g, "."));
    const compact = parts.join("").replace(/\.+/g, ".").replace(/^\.|\.$/g, "");
    return compact || fallback;
  }
  return fallback;
}

function summarizeTemplateSuggestion(target) {
  if (!target) {
    return null;
  }
  if (typeof target === "string") {
    return target;
  }
  if (target.template) {
    return renderTemplateSuggestion(target.template, target.expressions ?? []);
  }
  return JSON.stringify(target);
}

function renderTemplateSuggestion(parts, expressions) {
  let rendered = "";
  for (let index = 0; index < parts.length; index += 1) {
    rendered += parts[index];
    if (index < expressions.length) {
      rendered += expressionPlaceholder(expressions[index]);
    }
  }
  return rendered;
}

function expressionPlaceholder(value) {
  if (value === "UID" || value === "TENANT_ID" || value === "ROLES" || value === "PERMISSION_VERSION") {
    return `{${value}}`;
  }
  if (typeof value === "string" && /^[a-zA-Z_$][a-zA-Z0-9_$]*$/.test(value)) {
    return `{args.${value}}`;
  }
  return "{...}";
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const report = extractProject(options.cwd);
  const payload = options.includeSuggestions
    ? report
    : {
        root: report.root,
        scannedFiles: report.scannedFiles,
        operations: report.operations
      };

  let rendered;
  if (options.format !== "json") {
    throw new Error(`Unsupported format: ${options.format}`);
  }

  rendered = JSON.stringify(payload, null, 2);

  if (options.output) {
    fs.writeFileSync(options.output, rendered);
  } else {
    process.stdout.write(`${rendered}\n`);
  }
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exit(1);
}
