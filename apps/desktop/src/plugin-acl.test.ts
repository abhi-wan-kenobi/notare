import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
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

function capabilityGrants(): Set<string> {
  const dir = join(desktopRoot, "src-tauri/capabilities");
  const grants = new Set<string>();
  for (const file of readdirSync(dir).filter((f) => f.endsWith(".json"))) {
    const capability = JSON.parse(readFileSync(join(dir, file), "utf8")) as {
      permissions: Array<string | { identifier: string }>;
    };
    for (const permission of capability.permissions) {
      grants.add(
        typeof permission === "string" ? permission : permission.identifier,
      );
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
};

function loadPlugins(): Map<string, PluginAcl> {
  const plugins = new Map<string, PluginAcl>();
  for (const dir of readdirSync(pluginsRoot)) {
    const pluginDir = join(pluginsRoot, dir);
    const bindingsPath = join(pluginDir, "js/bindings.gen.ts");
    const packagePath = join(pluginDir, "package.json");
    if (!existsSync(bindingsPath) || !existsSync(packagePath)) continue;

    const methods = new Map<string, string>();
    let name: string | undefined;
    for (const chunk of readFileSync(bindingsPath, "utf8").split(
      /\n(?=async )/,
    )) {
      const method = /^async (\w+)\(/.exec(chunk)?.[1];
      const invoke = /TAURI_INVOKE\("plugin:([\w-]+)\|(\w+)"/.exec(chunk);
      if (!method || !invoke) continue;
      name = invoke[1];
      methods.set(method, invoke[2]);
    }
    if (!name) continue;

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
    });
  }
  return plugins;
}

// Every `<alias>.<method>(` call on a plugin's `commands` export, per source file.
function calledCommands(plugins: Map<string, PluginAcl>) {
  const calls: Array<{ file: string; plugin: PluginAcl; command: string }> = [];
  const sources = walk(
    join(desktopRoot, "src"),
    (p) => /\.tsx?$/.test(p) && !/\.test\.tsx?$/.test(p),
  );
  for (const file of sources) {
    const text = readFileSync(file, "utf8");
    for (const match of text.matchAll(
      /import\s*(?:type\s*)?\{([^}]*)\}\s*from\s*"(@hypr\/plugin-[\w-]+)"/g,
    )) {
      const plugin = plugins.get(match[2]);
      const alias = /(?:^|[\s,])commands(?:\s+as\s+(\w+))?\s*(?:,|$)/.exec(
        match[1],
      );
      if (!plugin || !alias) continue;
      const local = alias[1] ?? "commands";
      for (const call of text.matchAll(
        new RegExp(`\\b${local}\\s*\\.\\s*(\\w+)\\s*\\(`, "g"),
      )) {
        const command = plugin.methods.get(call[1]);
        if (command) {
          calls.push({
            file: file.slice(repoRoot.length + 1),
            plugin,
            command,
          });
        }
      }
    }
  }
  return calls;
}

describe("plugin command ACL", () => {
  const grants = capabilityGrants();
  const plugins = loadPlugins();
  const calls = calledCommands(plugins);

  it("finds the frontend's plugin command calls", () => {
    // Guards the parser itself: an empty scan would pass vacuously.
    expect(calls.length).toBeGreaterThan(50);
    expect(
      calls.some(
        (c) =>
          c.plugin.name === "transcription" && c.command === "run_diarization",
      ),
    ).toBe(true);
  });

  it("grants every plugin command the desktop frontend calls", () => {
    const denied = new Set<string>();
    for (const { file, plugin, command } of calls) {
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
});
