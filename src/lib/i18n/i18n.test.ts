import { describe, expect, it } from "vitest";
import { en } from "./en";
import { vi as viLocale } from "./vi";

function collectShape(obj: unknown, prefix = ""): Record<string, string> {
  const out: Record<string, string> = {};
  if (obj && typeof obj === "object" && !Array.isArray(obj)) {
    for (const [k, v] of Object.entries(obj)) {
      const path = prefix ? `${prefix}.${k}` : k;
      if (v && typeof v === "object" && !Array.isArray(v)) {
        Object.assign(out, collectShape(v, path));
      } else {
        out[path] = typeof v;
      }
    }
  }
  return out;
}

describe("i18n translation parity", () => {
  it("keeps identical keys and value types between en and vi", () => {
    const enShape = collectShape(en);
    const viShape = collectShape(viLocale);

    expect(Object.keys(viShape).sort()).toEqual(Object.keys(enShape).sort());
    for (const key of Object.keys(enShape)) {
      expect(viShape[key], `Type mismatch for i18n key '${key}'`).toBe(enShape[key]);
    }
  });
});
