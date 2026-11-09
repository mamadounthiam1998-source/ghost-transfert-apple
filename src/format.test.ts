import { describe, expect, it } from "vitest";
import { fmtBytes, fmtEta, fmtPercent, fmtSpeed } from "./format";

describe("format", () => {
  it("formats bytes in decimal units", () => {
    expect(fmtBytes(0)).toBe("0 o");
    expect(fmtBytes(1500)).toBe("1.5 Ko");
    expect(fmtBytes(256_000_000_000)).toBe("256 Go");
    expect(fmtBytes(null)).toBe("—");
  });
  it("formats eta and speed", () => {
    expect(fmtEta(null)).toBe("—");
    expect(fmtEta(42)).toBe("42 s");
    expect(fmtEta(125)).toBe("2 min 5 s");
    expect(fmtEta(7300)).toBe("2 h 1 min");
    expect(fmtSpeed(0)).toBe("—");
    expect(fmtSpeed(2_000_000)).toBe("2.0 Mo/s");
  });
  it("clamps percentages", () => {
    expect(fmtPercent(-5)).toBe("0 %");
    expect(fmtPercent(250)).toBe("100 %");
    expect(fmtPercent(45.67)).toBe("45.7 %");
  });
});
