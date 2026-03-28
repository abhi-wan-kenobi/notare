#!/usr/bin/env node

const { execSync } = require("node:child_process");
const { existsSync, mkdirSync } = require("node:fs");
const { join } = require("node:path");

const REPO = "fastrepl/char";

const TARGETS = {
  "darwin-arm64": "aarch64-apple-darwin",
  "darwin-x64": "x86_64-apple-darwin",
};

const target = TARGETS[`${process.platform}-${process.arch}`];
if (!target) {
  console.error(`Unsupported platform: ${process.platform}-${process.arch}`);
  process.exit(1);
}

const version = require("./package.json").version;
const tag = `cli_v${version}`;
const archive = `char-${version}-${target}.tar.xz`;
const url = `https://github.com/${REPO}/releases/download/${tag}/${archive}`;

const binDir = join(__dirname, "bin");
if (!existsSync(binDir)) {
  mkdirSync(binDir, { recursive: true });
}

const dest = join(binDir, "char");
if (existsSync(dest)) {
  downloadCliUI();
  process.exit(0);
}

console.error(`Downloading char from ${url}`);
const tmp = join(require("node:os").tmpdir(), archive);
execSync(`curl -fsSL -o "${tmp}" "${url}"`, { stdio: "inherit" });
execSync(`tar -xf "${tmp}" -C "${binDir}"`, { stdio: "inherit" });
execSync(`rm -f "${tmp}"`);
execSync(`chmod +x "${dest}"`);

downloadCliUI();

function downloadCliUI() {
  if (process.platform !== "darwin") return;

  const uiDest = join(binDir, "char-cli-ui");
  if (existsSync(uiDest)) return;

  const uiArchive = `char-cli-ui-${version}-${target}.tar.xz`;
  const uiUrl = `https://github.com/${REPO}/releases/download/${tag}/${uiArchive}`;

  console.error(`Downloading char-cli-ui from ${uiUrl}`);
  try {
    const uiTmp = join(require("node:os").tmpdir(), uiArchive);
    execSync(`curl -fsSL -o "${uiTmp}" "${uiUrl}"`, { stdio: "inherit" });
    execSync(`tar -xf "${uiTmp}" -C "${binDir}"`, { stdio: "inherit" });
    execSync(`rm -f "${uiTmp}"`);
    execSync(`chmod +x "${uiDest}"`);
  } catch {
    console.error(
      "Warning: failed to download char-cli-ui (shortcut will not work)",
    );
  }
}
