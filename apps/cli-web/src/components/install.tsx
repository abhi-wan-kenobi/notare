import { useState } from "react";

const INSTALL_CMD = "npm install -g @anthropic/char";

export function Install() {
  const [copied, setCopied] = useState(false);

  function copy() {
    navigator.clipboard.writeText(INSTALL_CMD);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }

  return (
    <section className="mb-16">
      <p className="mb-3 text-sm text-neutral-500">Run this to get started:</p>
      <button
        type="button"
        onClick={copy}
        className={[
          "flex w-full items-center justify-between",
          "rounded-lg border border-neutral-800 bg-neutral-900 px-5 py-4",
          "font-mono text-sm text-neutral-200",
          "transition-colors hover:border-neutral-700",
          "cursor-pointer",
        ].join(" ")}
      >
        <span>$ {INSTALL_CMD}</span>
        <span className="text-xs text-neutral-500">
          {copied ? "copied" : "copy"}
        </span>
      </button>
    </section>
  );
}
