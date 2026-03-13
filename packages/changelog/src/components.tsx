import { cn } from "@hypr/utils";

export const changelogComponents = {
  h2: ({ children }: { children?: React.ReactNode }) => (
    <h2 className="mt-8 mb-4 text-2xl font-semibold text-stone-800">
      {children}
    </h2>
  ),
  h3: ({ children }: { children?: React.ReactNode }) => (
    <h3 className="mt-6 mb-3 text-xl font-semibold text-stone-800">
      {children}
    </h3>
  ),
  h4: ({ children }: { children?: React.ReactNode }) => (
    <h4 className="mt-4 mb-2 text-lg font-semibold text-stone-800">
      {children}
    </h4>
  ),
  ul: ({ children }: { children?: React.ReactNode }) => (
    <ul className="my-2 list-disc pl-6 text-stone-700">{children}</ul>
  ),
  ol: ({ children }: { children?: React.ReactNode }) => (
    <ol className="my-2 list-decimal pl-6 text-stone-700">{children}</ol>
  ),
  li: ({ children }: { children?: React.ReactNode }) => (
    <li className="my-0.5 text-stone-700">{children}</li>
  ),
  p: ({ children }: { children?: React.ReactNode }) => (
    <p className="my-2 text-stone-700">{children}</p>
  ),
  strong: ({ children }: { children?: React.ReactNode }) => (
    <strong className="font-semibold text-stone-900">{children}</strong>
  ),
  em: ({ children }: { children?: React.ReactNode }) => (
    <em className="italic">{children}</em>
  ),
  blockquote: ({ children }: { children?: React.ReactNode }) => (
    <blockquote className="my-4 border-l-2 border-stone-300 pl-4 text-stone-600 italic">
      {children}
    </blockquote>
  ),
  a: ({ href, children }: { href?: string; children?: React.ReactNode }) => (
    <a
      className="text-stone-700 underline decoration-dotted hover:text-stone-800"
      href={href}
      target="_blank"
      rel="noopener noreferrer"
    >
      {children}
    </a>
  ),
  img: ({ src, alt }: { src?: string; alt?: string }) => (
    <img
      src={src}
      alt={alt}
      className="my-6 rounded-lg border border-neutral-200"
    />
  ),
  banner: ({
    title,
    variant,
    children,
  }: {
    title?: string;
    variant?: string;
    children?: React.ReactNode;
  }) => (
    <div
      className={cn([
        "my-4 rounded-lg border p-4",
        variant === "warning"
          ? "border-amber-300 bg-amber-50 text-amber-900"
          : "border-blue-300 bg-blue-50 text-blue-900",
      ])}
    >
      {title && <div className="mb-1 font-semibold">{title}</div>}
      <div className="text-sm">{children}</div>
    </div>
  ),
};
