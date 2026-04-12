---
name: new-changelog
description: Create the next desktop changelog entry when asked to add a changelog file or prepare the next release note under `packages/changelog/content`. Use this when the task is specifically about determining the next version and creating the markdown entry.
---

Determine the next desktop version by inspecting `.github/workflows/desktop_cd.yaml` and running:

```bash
doxxer --config doxxer.desktop.toml
```

Create the new markdown file in `packages/changelog/content` for that version.
