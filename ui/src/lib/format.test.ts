import { describe, it, expect } from "vitest";
import { humanBytes } from "./format";

describe("humanBytes", () => {
  it("formats bytes, MB and GB", () => {
    expect(humanBytes(500)).toBe("500 Б");
    expect(humanBytes(1024 * 1024)).toBe("1.0 МБ");
    expect(humanBytes(3 * 1024 ** 3)).toBe("3.0 ГБ");
  });
});
