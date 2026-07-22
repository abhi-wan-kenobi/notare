import { describe, expect, test } from "vitest";

import { parseCustomSttModels } from "./list-custom-stt";

describe("parseCustomSttModels", () => {
  // Regression guard for the shipped-in-#50 bug: the server serializes
  // `ModelIntegrity` as a serde-tagged object (`{ state: "verified" }`), not a
  // bare string, so `installed`/`corrupt` must be derived from `integrity.state`.
  // Reading it as a plain string left every model permanently "not installed",
  // which showed a Download icon on installed models and made rows unclickable.
  test("classifies installed/active from the tagged integrity object", () => {
    const raw = {
      models: [
        {
          id: "QuantizedLargeTurbo",
          displayName: "Large v3 Turbo (Q8)",
          sizeBytes: 874000000,
          englishOnly: false,
          active: true,
          integrity: { state: "verified" },
        },
        {
          id: "QuantizedSmall",
          displayName: "Small",
          active: false,
          integrity: { state: "notInstalled" },
        },
        {
          id: "QuantizedMedium",
          displayName: "Medium",
          active: false,
          integrity: { state: "presentUnverified" },
        },
        {
          id: "QuantizedBase",
          displayName: "Base",
          active: false,
          integrity: { state: "corrupt", detail: "checksum mismatch" },
        },
      ],
    };

    const models = parseCustomSttModels(raw);
    expect(models).toHaveLength(4);

    const byId = Object.fromEntries(models.map((m) => [m.id, m]));
    // verified → installed, and honours `active`
    expect(byId.QuantizedLargeTurbo).toMatchObject({
      installed: true,
      corrupt: false,
      active: true,
    });
    // notInstalled → needs download
    expect(byId.QuantizedSmall).toMatchObject({
      installed: false,
      corrupt: false,
      active: false,
    });
    // presentUnverified → still usable, counts as installed
    expect(byId.QuantizedMedium).toMatchObject({
      installed: true,
      corrupt: false,
    });
    // corrupt → not installed, re-downloadable
    expect(byId.QuantizedBase).toMatchObject({
      installed: false,
      corrupt: true,
    });
  });

  test("tolerates a bare-string integrity (defensive fallback)", () => {
    const models = parseCustomSttModels({
      models: [{ id: "X", integrity: "verified", active: false }],
    });
    expect(models[0]).toMatchObject({ installed: true, corrupt: false });
  });

  test("returns [] for a malformed payload", () => {
    expect(parseCustomSttModels(null)).toEqual([]);
    expect(parseCustomSttModels({})).toEqual([]);
    expect(parseCustomSttModels({ models: "nope" })).toEqual([]);
  });
});
