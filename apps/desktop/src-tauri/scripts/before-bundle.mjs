import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));

function runScript(scriptName, args = []) {
  return new Promise((resolve, reject) => {
    const child = spawn("bash", [path.join(scriptDir, scriptName), ...args], {
      stdio: "inherit",
    });

    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) {
        resolve();
        return;
      }

      reject(new Error(`${scriptName} exited with code ${code ?? "unknown"}`));
    });
  });
}

if (process.platform === "win32") {
  console.log("[before-bundle] Windows detected, skipping shell bundle hooks.");
  process.exit(0);
}

if (process.platform === "darwin") {
  await runScript("compile-icons.sh");
}

if (process.platform === "linux") {
  // Tauri's beforeBundleCommand passes no arguments, so hand fix-dylib.sh the
  // cargo release output dir explicitly. It patchelfs the app binary's RPATH to
  // the deb's private lib dir (+ $ORIGIN) and the bundled sherpa/onnxruntime
  // .so files to $ORIGIN, so the installed .deb resolves them (the .deb bundler,
  // unlike the AppImage's linuxdeploy, does not bundle or fix these libs).
  const releaseDir = path.join(scriptDir, "..", "target", "release");
  await runScript("fix-dylib.sh", [releaseDir]);
} else {
  await runScript("fix-dylib.sh");
}
