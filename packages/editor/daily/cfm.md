# CFM (Char-Flavored-Markdown)

`CFM` is what `Char` use for its `dailynote` edior.

## Comparasion

### vs Commonmark

- It is superset of [`Commonmark`](https://commonmark.org/help).

## vs GFM(Github-Flavored-Markodwn)

- The most important piece of CFM is how it represent `task` (more on that below)
- Note that representation like `- [ ]` is from [`GFM`](https://github.github.com/gfm), not `Commonmark`.

## What is Task

- Consists of `name`, `state`(at least 3, todo, in-progress, done), `due`, `trace`, and other `metadata` like remote job id.
- Need `tasks` table to manage this

## How Task represented

- `HTML` is allowed in `Commonmark`.
- `<task id="<id>"></task>
- This assume it can fetch info from `tasks` table to render UI.

## Other Consideration

- [Block-level Last-write-win](https://github.com/sqliteai/sqlite-sync/blob/1214933c940f0e3c36a610d101858cac1ee86cbc/docs/BLOCK-LWW.md?plain=1#L1
)
