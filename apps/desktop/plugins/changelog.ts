import { readFileSync, watch } from "node:fs";
import { resolve } from "node:path";
import type { Plugin, ViteDevServer } from "vite";

const __dirname = import.meta.dirname;
const changelogDir = resolve(__dirname, "../../../packages/changelog/content");

const packageJsonPath = resolve(__dirname, "../package.json");

const VIRTUAL_ID = "virtual:changelog";
const RESOLVED_ID = "\0" + VIRTUAL_ID;

function readAppVersion(): string | null {
  try {
    const pkg = JSON.parse(readFileSync(packageJsonPath, "utf-8"));
    return typeof pkg.version === "string" ? pkg.version : null;
  } catch {
    return null;
  }
}

// The bundled notes are for the version the app is built as, not the highest
// file in the directory: the content dir also holds notes for versions that
// are newer in number than this app (e.g. inherited fork history).
export function buildChangelogModule(
  dir: string,
  version: string | null,
): string {
  let content: string | null = null;

  if (version) {
    try {
      content = readFileSync(resolve(dir, `${version}.md`), "utf-8");
    } catch {}
  }

  return [
    `export const latestVersion = ${JSON.stringify(version)};`,
    `export const latestContent = ${JSON.stringify(content)};`,
  ].join("\n");
}

export function changelog(): Plugin {
  return {
    name: "changelog",
    resolveId(id) {
      if (id === VIRTUAL_ID) return RESOLVED_ID;
    },
    load(id) {
      if (id === RESOLVED_ID) {
        return buildChangelogModule(changelogDir, readAppVersion());
      }
    },
    configureServer(server: ViteDevServer) {
      if (process.env.NODE_ENV === "test" || process.env.VITEST) {
        return;
      }

      try {
        watch(changelogDir, { recursive: true }, () => {
          const mod = server.moduleGraph.getModuleById(RESOLVED_ID);
          if (mod) {
            server.moduleGraph.invalidateModule(mod);
            server.ws.send({ type: "full-reload" });
          }
        });
      } catch {}
    },
  };
}
