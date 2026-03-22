interface OptionDoc {
  flags: string;
  value_name?: string;
  help?: string;
  default?: string;
  required: boolean;
  is_flag: boolean;
}

interface ArgumentDoc {
  name: string;
  help?: string;
  default?: string;
  required: boolean;
}

interface CommandDoc {
  name: string;
  about?: string;
  synopsis: string;
  global_options?: OptionDoc[];
  options?: OptionDoc[];
  arguments?: ArgumentDoc[];
  subcommands?: CommandDoc[];
}

function anchorId(name: string) {
  return name.replace(/\s+/g, "-");
}

function OptionList({ title, items }: { title: string; items: OptionDoc[] }) {
  if (items.length === 0) return null;

  return (
    <div className="mb-4">
      <h4 className="mb-2 text-xs font-medium tracking-wide text-neutral-500 uppercase">
        {title}
      </h4>
      <div className="space-y-1">
        {items.map((opt) => (
          <div key={opt.flags} className="flex gap-3 text-sm">
            <code className="shrink-0 text-neutral-300">
              {opt.flags}
              {opt.value_name && !opt.is_flag && (
                <span className="text-neutral-500">
                  {" "}
                  &lt;{opt.value_name}&gt;
                </span>
              )}
            </code>
            <span className="text-neutral-500">
              {opt.help}
              {opt.default && (
                <span className="ml-1 text-neutral-600">
                  (default: {opt.default})
                </span>
              )}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function ArgumentList({ items }: { items: ArgumentDoc[] }) {
  if (items.length === 0) return null;

  return (
    <div className="mb-4">
      <h4 className="mb-2 text-xs font-medium tracking-wide text-neutral-500 uppercase">
        Arguments
      </h4>
      <div className="space-y-1">
        {items.map((arg) => (
          <div key={arg.name} className="flex gap-3 text-sm">
            <code className="shrink-0 text-neutral-300">
              &lt;{arg.name}&gt;
            </code>
            <span className="text-neutral-500">
              {arg.help}
              {arg.default && (
                <span className="ml-1 text-neutral-600">
                  (default: {arg.default})
                </span>
              )}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function Command({ cmd, depth }: { cmd: CommandDoc; depth: number }) {
  const id = anchorId(cmd.name);
  const Tag = depth === 0 ? "h2" : "h3";
  const subs = cmd.subcommands ?? [];

  return (
    <section id={id} className="mb-10">
      <Tag className="mb-1 font-mono text-base font-semibold text-neutral-100">
        <a href={`#${id}`} className="hover:text-white">
          {cmd.name}
        </a>
      </Tag>

      {cmd.about && (
        <p className="mb-3 text-sm text-neutral-400">{cmd.about}</p>
      )}

      <pre className="mb-4 overflow-x-auto rounded border border-neutral-800 bg-neutral-900 px-4 py-3 font-mono text-sm text-neutral-300">
        {cmd.synopsis}
      </pre>

      <OptionList title="Global options" items={cmd.global_options ?? []} />
      <OptionList title="Options" items={cmd.options ?? []} />
      <ArgumentList items={cmd.arguments ?? []} />

      {subs.length > 0 && (
        <div className="mb-4">
          <h4 className="mb-2 text-xs font-medium tracking-wide text-neutral-500 uppercase">
            Subcommands
          </h4>
          <div className="space-y-1 text-sm">
            {subs.map((s) => (
              <div key={s.name}>
                <a
                  href={`#${anchorId(s.name)}`}
                  className="font-mono text-neutral-300 hover:text-white"
                >
                  {s.name.split(" ").pop()}
                </a>
                {s.about && (
                  <span className="ml-2 text-neutral-500">— {s.about}</span>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {subs.length > 0 && (
        <div className="mt-8 border-t border-neutral-800 pt-8">
          {subs.map((s) => (
            <Command key={s.name} cmd={s} depth={depth + 1} />
          ))}
        </div>
      )}
    </section>
  );
}

export function CommandRef({ data }: { data: CommandDoc }) {
  return (
    <section className="mb-16">
      <h2 className="mb-8 text-sm font-medium tracking-wide text-neutral-500 uppercase">
        Reference
      </h2>
      <Command cmd={data} depth={0} />
    </section>
  );
}
