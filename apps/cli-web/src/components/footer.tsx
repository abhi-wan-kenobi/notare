export function Footer() {
  return (
    <footer className="border-t border-neutral-800 pt-8 text-sm text-neutral-500">
      <div className="flex gap-4">
        <a href="https://char.com" className="hover:text-neutral-300">
          char.com
        </a>
        <a
          href="https://github.com/anthropics/char"
          className="hover:text-neutral-300"
        >
          GitHub
        </a>
        <a href="https://char.com/docs" className="hover:text-neutral-300">
          Docs
        </a>
      </div>
    </footer>
  );
}
