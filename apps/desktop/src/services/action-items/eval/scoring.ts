/**
 * Scoring for the action-item golden eval (WS-C PR17).
 *
 * Pure: given the expected items for a transcript and the items the pipeline
 * actually kept, compute precision / recall / hallucination-rate. The release
 * gate is enforced against these:
 *   - hallucinated_source_text = 0  (structural; the substring gate guarantees
 *     it, the eval CONFIRMS it),
 *   - precision >= 0.8.
 */

import { normalizeForMatch } from "../gates";

export type ExpectedItem = {
  /** A verbatim quote (or distinctive fragment) that the correct item's
   * source_text should contain — used to match actual↔expected. */
  source_fragment: string;
  /** Expected owner speaker id, or "" for a correctly null owner. */
  owner_speaker_id: string;
  /** Optional expected due date (ISO) for date-resolution scoring. */
  due_at?: string;
};

export type ActualItem = {
  source_text: string;
  owner_speaker_id: string;
  due_at: string;
};

export type CaseScore = {
  name: string;
  truePositives: number;
  falsePositives: number;
  falseNegatives: number;
  /** kept items whose source_text isn't in the transcript (must be 0). */
  hallucinated: number;
  /** matched items where the owner was correct. */
  ownerCorrect: number;
  /** matched items where due_at was correct (of those with an expected date). */
  dueCorrect: number;
  dueExpected: number;
};

/** An actual item matches an expected item when the expected fragment is a
 * substring of the actual source_text (both normalized). */
function matches(actual: ActualItem, expected: ExpectedItem): boolean {
  const a = normalizeForMatch(actual.source_text);
  const frag = normalizeForMatch(expected.source_fragment);
  return frag.length > 0 && a.includes(frag);
}

export function scoreCase(params: {
  name: string;
  transcript: string;
  expected: ExpectedItem[];
  actual: ActualItem[];
}): CaseScore {
  const normTranscript = normalizeForMatch(params.transcript);
  const usedExpected = new Set<number>();
  let tp = 0;
  let ownerCorrect = 0;
  let dueCorrect = 0;
  let dueExpected = 0;
  let hallucinated = 0;

  for (const actual of params.actual) {
    if (!normTranscript.includes(normalizeForMatch(actual.source_text))) {
      hallucinated += 1;
    }
    // Greedy first-unused match.
    const idx = params.expected.findIndex(
      (e, i) => !usedExpected.has(i) && matches(actual, e),
    );
    if (idx >= 0) {
      usedExpected.add(idx);
      tp += 1;
      const exp = params.expected[idx];
      if (actual.owner_speaker_id === exp.owner_speaker_id) ownerCorrect += 1;
      if (exp.due_at !== undefined) {
        dueExpected += 1;
        if (actual.due_at === exp.due_at) dueCorrect += 1;
      }
    }
  }

  return {
    name: params.name,
    truePositives: tp,
    falsePositives: params.actual.length - tp,
    falseNegatives: params.expected.length - tp,
    hallucinated,
    ownerCorrect,
    dueCorrect,
    dueExpected,
  };
}

export type EvalReport = {
  cases: CaseScore[];
  precision: number;
  recall: number;
  hallucinated: number;
  ownerAccuracy: number;
  dueAccuracy: number;
  /** Release-gate verdict. */
  pass: boolean;
  failures: string[];
};

export const PRECISION_GATE = 0.8;

export function aggregate(cases: CaseScore[]): EvalReport {
  const sum = (f: (c: CaseScore) => number) =>
    cases.reduce((a, c) => a + f(c), 0);
  const tp = sum((c) => c.truePositives);
  const fp = sum((c) => c.falsePositives);
  const fn = sum((c) => c.falseNegatives);
  const hallucinated = sum((c) => c.hallucinated);
  const ownerCorrect = sum((c) => c.ownerCorrect);
  const dueCorrect = sum((c) => c.dueCorrect);
  const dueExpected = sum((c) => c.dueExpected);

  const precision = tp + fp === 0 ? 1 : tp / (tp + fp);
  const recall = tp + fn === 0 ? 1 : tp / (tp + fn);
  const ownerAccuracy = tp === 0 ? 1 : ownerCorrect / tp;
  const dueAccuracy = dueExpected === 0 ? 1 : dueCorrect / dueExpected;

  const failures: string[] = [];
  if (hallucinated > 0) {
    failures.push(
      `RELEASE GATE FAILED: ${hallucinated} hallucinated source_text (must be 0)`,
    );
  }
  if (precision < PRECISION_GATE) {
    failures.push(`precision ${precision.toFixed(3)} < ${PRECISION_GATE} gate`);
  }

  return {
    cases,
    precision,
    recall,
    hallucinated,
    ownerAccuracy,
    dueAccuracy,
    pass: failures.length === 0,
    failures,
  };
}

export function formatReport(report: EvalReport): string {
  const pct = (n: number) => `${(n * 100).toFixed(1)}%`;
  const lines = [
    "Action-item extraction eval",
    "===========================",
    `precision:      ${pct(report.precision)}  (gate >= ${pct(PRECISION_GATE)})`,
    `recall:         ${pct(report.recall)}`,
    `owner accuracy: ${pct(report.ownerAccuracy)}`,
    `due accuracy:   ${pct(report.dueAccuracy)}`,
    `hallucinated:   ${report.hallucinated}  (gate = 0)`,
    "",
    report.pass ? "PASS" : `FAIL\n - ${report.failures.join("\n - ")}`,
  ];
  return lines.join("\n");
}
