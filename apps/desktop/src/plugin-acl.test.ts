import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import ts from "typescript";
import { describe, expect, it } from "vitest";

// Tauri denies any plugin command that no capability grants, and every caller
// here catches the rejection, so a missing grant ships as a feature that
// silently never runs (the 0.5.0-rc2 webhook page, on-device diarization,
// semantic search). This test reads the capability files, each plugin's
// permission files and generated bindings, and fails for every plugin command
// the desktop frontend calls without a grant.

const repoRoot = existsSync(resolve(process.cwd(), "src-tauri/capabilities"))
  ? resolve(process.cwd(), "../..")
  : process.cwd();
const desktopRoot = join(repoRoot, "apps/desktop");
const pluginsRoot = join(repoRoot, "plugins");

function walk(dir: string, accept: (path: string) => boolean): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) {
      return entry === "node_modules" ? [] : walk(path, accept);
    }
    return accept(path) ? [path] : [];
  });
}

function parse(path: string): ts.SourceFile {
  return ts.createSourceFile(
    path,
    readFileSync(path, "utf8"),
    ts.ScriptTarget.Latest,
    true,
    path.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
  );
}

function visit(node: ts.Node, fn: (node: ts.Node) => void) {
  fn(node);
  ts.forEachChild(node, (child) => visit(child, fn));
}

function capabilityGrants(): Set<string> {
  const dir = join(desktopRoot, "src-tauri/capabilities");
  const grants = new Set<string>();
  for (const file of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
    const capability: unknown = JSON.parse(
      readFileSync(join(dir, file), "utf8"),
    );
    if (
      !capability ||
      typeof capability !== "object" ||
      !("permissions" in capability) ||
      !Array.isArray(capability.permissions)
    ) {
      continue;
    }
    for (const permission of capability.permissions) {
      if (typeof permission === "string") {
        grants.add(permission);
      } else if (
        permission &&
        typeof permission === "object" &&
        "identifier" in permission &&
        typeof permission.identifier === "string"
      ) {
        grants.add(permission.identifier);
      }
    }
  }
  return grants;
}

type PluginAcl = {
  name: string;
  defaultPermissions: Set<string>;
  definedPermissions: Set<string>;
  // binding method name -> command name
  methods: Map<string, string>;
  // TAURI_INVOKE calls whose enclosing binding name could not be read
  unmapped: string[];
};

function declaredName(fn: ts.Node): string | undefined {
  const named =
    ts.isMethodDeclaration(fn) || ts.isFunctionDeclaration(fn)
      ? fn
      : ts.isPropertyAssignment(fn.parent) ||
          ts.isVariableDeclaration(fn.parent) ||
          ts.isPropertyDeclaration(fn.parent)
        ? fn.parent
        : undefined;
  return named?.name && ts.isIdentifier(named.name)
    ? named.name.text
    : undefined;
}

// Every `TAURI_INVOKE("plugin:<name>|<command>", ...)` in a bindings file,
// keyed by the binding that wraps it.
function bindingInvokes(path: string) {
  const invokes: Array<{ plugin: string; command: string; method?: string }> =
    [];
  visit(parse(path), (node) => {
    if (
      !ts.isCallExpression(node) ||
      !ts.isIdentifier(node.expression) ||
      node.expression.text !== "TAURI_INVOKE"
    ) {
      return;
    }
    const target = node.arguments[0];
    const invoke =
      target && ts.isStringLiteralLike(target)
        ? /^plugin:([\w-]+)\|(\w+)$/.exec(target.text)
        : null;
    if (!invoke) return;
    // The outermost named function is the binding the frontend calls; an
    // inner helper inside it must not take its place.
    let method: string | undefined;
    for (let fn: ts.Node | undefined = node.parent; fn; fn = fn.parent) {
      if (ts.isFunctionLike(fn)) method = declaredName(fn) ?? method;
    }
    invokes.push({ plugin: invoke[1], command: invoke[2], method });
  });
  return invokes;
}

function loadPlugins(): Map<string, PluginAcl> {
  const plugins = new Map<string, PluginAcl>();
  for (const dir of readdirSync(pluginsRoot)) {
    const pluginDir = join(pluginsRoot, dir);
    const bindingsPath = join(pluginDir, "js/bindings.gen.ts");
    const packagePath = join(pluginDir, "package.json");
    if (!existsSync(bindingsPath) || !existsSync(packagePath)) continue;

    const invokes = bindingInvokes(bindingsPath);
    const name = invokes[0]?.plugin;
    if (!name) continue;
    const methods = new Map<string, string>();
    const unmapped: string[] = [];
    for (const { command, method } of invokes) {
      if (method) methods.set(method, command);
      else unmapped.push(command);
    }

    const permissionFiles = existsSync(join(pluginDir, "permissions"))
      ? walk(join(pluginDir, "permissions"), (p) => p.endsWith(".toml"))
      : [];
    const definedPermissions = new Set<string>();
    const defaultPermissions = new Set<string>();
    for (const file of permissionFiles) {
      const text = readFileSync(file, "utf8");
      for (const match of text.matchAll(/identifier\s*=\s*"([^"]+)"/g)) {
        definedPermissions.add(match[1]);
      }
      const defaultBlock =
        /\[default\][\s\S]*?permissions\s*=\s*\[([^\]]*)\]/.exec(text);
      for (const match of defaultBlock?.[1].matchAll(/"([^"]+)"/g) ?? []) {
        defaultPermissions.add(match[1]);
      }
    }

    const packageJson: unknown = JSON.parse(readFileSync(packagePath, "utf8"));
    if (
      !packageJson ||
      typeof packageJson !== "object" ||
      !("name" in packageJson) ||
      typeof packageJson.name !== "string"
    ) {
      continue;
    }
    plugins.set(packageJson.name, {
      name,
      defaultPermissions,
      definedPermissions,
      methods,
      unmapped,
    });
  }
  return plugins;
}

type Scan = {
  uses: Array<{ file: string; plugin: PluginAcl; command: string }>;
  // Ways of reaching a plugin's commands that the scan cannot follow.
  unscanned: string[];
};

// Walks every desktop source file: each `commands` import from a plugin
// package, then every reference to its local name. `<local>.<method>` (a call
// or a reference) is a use; any other reference would hide calls from the
// scan, so it is reported.
function scanDesktopSources(plugins: Map<string, PluginAcl>): Scan {
  const scan: Scan = { uses: [], unscanned: [] };
  const sources = walk(
    join(desktopRoot, "src"),
    (p) => /\.tsx?$/.test(p) && !/\.test\.tsx?$/.test(p),
  );
  for (const path of sources) {
    const file = path.slice(repoRoot.length + 1);
    const source = parse(path);
    const locals = new Map<string, PluginAcl>();

    for (const statement of source.statements) {
      if (
        !ts.isImportDeclaration(statement) ||
        !ts.isStringLiteral(statement.moduleSpecifier)
      ) {
        continue;
      }
      const packageName = /^@hypr\/plugin-[\w-]+/.exec(
        statement.moduleSpecifier.text,
      )?.[0];
      const plugin = packageName ? plugins.get(packageName) : undefined;
      const clause = statement.importClause;
      if (!plugin || !clause) continue;
      if (clause.name) {
        scan.unscanned.push(`${file}: default import of ${plugin.name}`);
      }
      const bindings = clause.namedBindings;
      if (!bindings) continue;
      if (ts.isNamespaceImport(bindings)) {
        scan.unscanned.push(`${file}: namespace import of ${plugin.name}`);
        continue;
      }
      for (const element of bindings.elements) {
        if ((element.propertyName ?? element.name).text === "commands") {
          locals.set(element.name.text, plugin);
        }
      }
    }
    if (locals.size === 0) continue;

    visit(source, (node) => {
      if (!ts.isIdentifier(node)) return;
      const plugin = locals.get(node.text);
      if (!plugin) return;
      const parent = node.parent;
      // Import specifiers, `x.commands`, `{ commands: … }` and `typeof
      // commands` name the export, a property or a type, not a runtime use
      // of the local alias.
      if (
        ts.isImportSpecifier(parent) ||
        ts.isTypeQueryNode(parent) ||
        (ts.isPropertyAccessExpression(parent) && parent.name === node) ||
        (ts.isPropertyAssignment(parent) && parent.name === node)
      ) {
        return;
      }
      if (ts.isPropertyAccessExpression(parent) && parent.expression === node) {
        const command = plugin.methods.get(parent.name.text);
        if (command) {
          scan.uses.push({ file, plugin, command });
        } else {
          scan.unscanned.push(
            `${file}: ${node.text}.${parent.name.text} is not a ${plugin.name} binding`,
          );
        }
        return;
      }
      scan.unscanned.push(
        `${file}: \`${node.text}\` used other than as ${node.text}.<method>`,
      );
    });
  }
  return scan;
}

describe("plugin command ACL", () => {
  const grants = capabilityGrants();
  const plugins = loadPlugins();
  const { uses, unscanned } = scanDesktopSources(plugins);

  it("finds the frontend's plugin command calls", () => {
    // Guards the parser itself: an empty scan would pass vacuously.
    expect(uses.length).toBeGreaterThan(50);
    expect(
      uses.some(
        (c) =>
          c.plugin.name === "transcription" && c.command === "run_diarization",
      ),
    ).toBe(true);
    expect(
      [...plugins.values()].find((p) => p.name === "settings")?.methods.size,
    ).toBeGreaterThan(0);
    // A bindings shape the parser cannot name would hide that plugin's calls.
    expect(
      [...plugins.values()].flatMap((p) =>
        p.unmapped.map((command) => `${p.name}|${command}`),
      ),
    ).toEqual([]);
  });

  it("only reaches plugin commands through `<alias>.<method>`", () => {
    expect(unscanned.sort()).toEqual([]);
  });

  it("grants every plugin command the desktop frontend calls", () => {
    const denied = new Set<string>();
    for (const { file, plugin, command } of uses) {
      const permission = `allow-${command.replace(/_/g, "-")}`;
      const granted =
        grants.has(`${plugin.name}:${permission}`) ||
        (grants.has(`${plugin.name}:default`) &&
          plugin.defaultPermissions.has(permission));
      if (!granted) {
        denied.add(`${plugin.name}|${command} (called in ${file})`);
      }
    }
    expect([...denied].sort()).toEqual([]);
  });

  it("defines every permission a plugin's default set references", () => {
    // A default.toml entry with no generated permission file means the
    // command is missing from the plugin's build.rs COMMANDS list.
    const undefinedPermissions: string[] = [];
    for (const plugin of plugins.values()) {
      for (const permission of plugin.defaultPermissions) {
        if (!plugin.definedPermissions.has(permission)) {
          undefinedPermissions.push(`${plugin.name}:${permission}`);
        }
      }
    }
    expect(undefinedPermissions.sort()).toEqual([]);
  });

  it("defines a permission for every command a plugin exposes", () => {
    // A command in the bindings but not in build.rs has no permission, so no
    // capability can ever grant it; the first frontend call would be denied.
    const missing: string[] = [];
    for (const plugin of plugins.values()) {
      for (const command of plugin.methods.values()) {
        const permission = `allow-${command.replace(/_/g, "-")}`;
        if (!plugin.definedPermissions.has(permission)) {
          missing.push(`${plugin.name}:${permission}`);
        }
      }
    }
    expect(missing.sort()).toEqual([]);
  });
});
