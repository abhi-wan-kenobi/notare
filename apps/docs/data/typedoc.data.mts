import fs from "fs";
import os from "os";
import path from "path";

import { Application, ProjectReflection } from "typedoc";
import { defineLoader } from "vitepress";

import type { PluginFrontmatter } from "../.vitepress/types";
import plugins from "./plugins.data.mts";

export default defineLoader({
  async load(): Promise<Record<string, ProjectReflection>> {
    const ids = await plugins.load().then((data) => {
      return data.map(
        ({ frontmatter }) => (frontmatter as PluginFrontmatter).id,
      );
    });

    const ret = {};
    await Promise.all(
      ids.map(async (id) => {
        ret[id] = await writeReflection(id);
      }),
    );

    return ret;
  },
});

const writeReflection = async (id: string) => {
  const tempDir = path.resolve(os.tmpdir(), "hyprnote");

  const app = await Application.bootstrapWithPlugins({
    excludeExternals: true,
    tsconfig: path.resolve(__dirname, `../../../plugins/${id}/tsconfig.json`),
    entryPoints: [
      path.resolve(__dirname, `../../../plugins/${id}/js/index.ts`),
    ],
  });

  const project = await app.convert();
  if (project) {
    await app.generateJson(project, path.resolve(tempDir, `${id}.json`));
  }

  const data = JSON.parse(
    fs.readFileSync(path.resolve(tempDir, `${id}.json`), "utf-8"),
  );

  return data;
};
