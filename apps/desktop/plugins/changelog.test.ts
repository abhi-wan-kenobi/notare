import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { parseFrontmatter } from "@hypr/changelog";

import { buildChangelogModule, changelog } from "./changelog";

const dirs: string[] = [];

function contentDir(files: Record<string, string>) {
  const dir = mkdtempSync(join(tmpdir(), "changelog-"));
  dirs.push(dir);
  for (const [name, body] of Object.entries(files)) {
    writeFileSync(join(dir, name), body);
  }
  return dir;
}

function evalModule(source: string) {
  const exports: Record<string, unknown> = {};
  new Function(
    "exports",
    source.replace(/export const (\w+) =/g, "exports.$1 ="),
  )(exports);
  return exports as {
    latestVersion: string | null;
    latestContent: string | null;
  };
}

afterEach(() => {
  while (dirs.length) rmSync(dirs.pop()!, { recursive: true, force: true });
});

describe("virtual:changelog", () => {
  it("bundles the app version's notes even when a higher-numbered file exists", () => {
    const dir = contentDir({
      "0.6.7.md": "current",
      "1.1.16.md": "fork history",
    });

    const mod = evalModule(buildChangelogModule(dir, "0.6.7"));

    expect(mod.latestVersion).toBe("0.6.7");
    expect(mod.latestContent).toBe("current");
  });

  it("reports the version with null content when its file is missing", () => {
    const dir = contentDir({ "1.1.16.md": "fork history" });

    const mod = evalModule(buildChangelogModule(dir, "0.6.8"));

    expect(mod.latestVersion).toBe("0.6.8");
    expect(mod.latestContent).toBeNull();
  });

  it("resolves the real content directory to the real 0.6.7 notes through the plugin", () => {
    const plugin = changelog() as unknown as {
      resolveId: (id: string) => string | undefined;
      load: (id: string) => string | undefined;
    };
    const resolved = plugin.resolveId("virtual:changelog");
    const mod = evalModule(plugin.load(resolved!)!);

    const appVersion = JSON.parse(
      readFileSync(resolve(__dirname, "../package.json"), "utf-8"),
    ).version;

    expect(mod.latestVersion).toBe(appVersion);
    expect(mod.latestContent).not.toBeNull();
    expect(parseFrontmatter(mod.latestContent!).summary).toBeTruthy();
  });
});
