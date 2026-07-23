# Action-item extraction eval (WS-C PR17)

Golden-set eval that measures the extraction pipeline's **precision / recall /
owner+due accuracy** and confirms the structural **release gates**:

- **hallucinated `source_text` = 0** — guaranteed by the substring gate in
  `../gates.ts`; the eval confirms it and fails loudly if it's ever non-zero.
- **precision ≥ 0.8**.

## Pieces

- `fixtures.ts` — 8 golden cases covering the plan's categories: clean
  single-owner + relative date, no-items chit-chat, `"someone should really…"`
  (a non-commitment that must NOT become a task), implied owner, explicit ISO
  date, unassigned/null-owner task, cross-talk two-owners, and a question.
- `scoring.ts` — pure precision/recall/hallucination/owner/due scoring +
  `aggregate` (applies the gates) + `formatReport`.
- `run.ts` — `runEval(model, { generateObjectFn? })` runs the **real**
  `extractActionItems` pipeline over the fixtures and scores it.
- `eval.test.ts` — hermetic: a scripted "model" returns each case's golden items
  **plus a fabrication**; the run confirms the gate strips the fabrication
  (hallucination = 0) and precision clears the gate. Runs in CI, no network.

## Live run (against a real model, outside CI)

The hermetic test proves the *machinery*. To get real quality numbers against a
model, run the pipeline with an AI-SDK model:

```ts
import { createOpenAICompatible } from "@ai-sdk/openai-compatible";
import { runEval } from "./run";
import { formatReport } from "./scoring";

const ollama = createOpenAICompatible({ name: "ollama", baseURL: "http://127.0.0.1:11434/v1" });
const report = await runEval(ollama.chatModel("qwen3:8b"));
console.log(formatReport(report));
```

…pass an ollama target so the pipeline uses the native `format` endpoint:

```ts
const target = { providerId: "ollama", modelId: "qwen3:8b", baseUrl: "http://127.0.0.1:11434/v1" };
const report = await runEval({} as never, { target, fetchFn: fetch });
```

**Resolved (2026-07-23):** `qwen3:8b` via the AI-SDK `@ai-sdk/openai-compatible`
provider returned prose / empty content (reasoning models aren't grammar-
constrained on `/v1/chat/completions` `response_format`), so `generateObject`
raised `AI_NoObjectGeneratedError`. **Fix:** `structured-generate.ts` routes
ollama structured calls through the NATIVE `/api/chat` with `format: <json-schema>`
+ `think: false`, which grammar-constrains the decode → valid JSON every time.
`extract.ts` uses it whenever `deps.target.providerId === "ollama"`. The WS-A
runtime probe (`structured-capability.ts` wrapping `probeStructuredOutputs`)
remains the gate for *non*-ollama BYO endpoints (ollama is exempt — the native
path guarantees it). Live precision numbers now land; record them in the 0.5
Production Gate.
