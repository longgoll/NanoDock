import { describe, expect, it } from "vitest";
import type { HostCapabilities } from "../api/daemon";
import {
  getCpuCoreOptions,
  getMaxMemoryMb,
  getMemoryPresets,
  getProfileValues,
  isMemoryHighForHost,
} from "./wslRecommendation";

function makeHost(overrides: Partial<HostCapabilities> = {}): HostCapabilities {
  return {
    total_memory_mb: 16384,
    cpu_cores: 12,
    windows_build: 22631,
    windows_label: "Windows 11",
    wsl_version: "2.3.26",
    supports_mirrored_networking: true,
    supports_sparse_vhd: true,
    recommended_memory_mb: 4096,
    recommended_swap_mb: 4096,
    recommended_processors: null,
    recommended_mirrored_networking: true,
    recommended_sparse_vhd: true,
    ...overrides,
  };
}

describe("wslRecommendation helpers", () => {
  it("scales max memory and presets to host RAM", () => {
    const host8g = makeHost({ total_memory_mb: 8192, recommended_memory_mb: 2048 });
    expect(getMaxMemoryMb(host8g)).toBe(8192);
    expect(getMemoryPresets(host8g)).toEqual([2048, 4096, 6144, 8192]);

    const host32g = makeHost({ total_memory_mb: 32768, recommended_memory_mb: 8192 });
    expect(getMaxMemoryMb(host32g)).toBe(24576);
    expect(getMemoryPresets(host32g)).toContain(24576);
    expect(getMemoryPresets(host32g)).not.toContain(32768);
  });

  it("derives dynamic profiles and flags high memory usage", () => {
    const host16g = makeHost({ total_memory_mb: 16384, recommended_memory_mb: 4096 });
    expect(getProfileValues("balanced", host16g).memory_mb).toBe(4096);
    expect(getProfileValues("perf", host16g).memory_mb).toBe(8192);

    expect(isMemoryHighForHost(8192, host16g)).toBe(false);
    expect(isMemoryHighForHost(12288, host16g)).toBe(true);
  });

  it("limits CPU options to host logical cores", () => {
    const host = makeHost({ cpu_cores: 10 });
    expect(getCpuCoreOptions(host)).toEqual([2, 4, 6, 8, 10]);
  });
});
