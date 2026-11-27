import type { HostCapabilities } from "../api/daemon";

export type ResourceProfileKind = "eco" | "balanced" | "perf";

export interface ResourceProfileValues {
  memory_mb: number;
  swap_mb: number;
  idle_timeout_secs: number;
}

const ALL_MEMORY_PRESETS = [2048, 4096, 6144, 8192, 12288, 16384, 24576, 32768];
const ALL_CPU_OPTIONS = [2, 4, 6, 8, 12, 16, 24, 32];

function roundToStep(value: number, step: number): number {
  return Math.round(value / step) * step;
}

/**
 * Computes the safe upper bound for the RAM slider based on host physical RAM.
 */
export function getMaxMemoryMb(host: HostCapabilities | null): number {
  if (!host || host.total_memory_mb <= 0) {
    return 32768;
  }
  const seventyFivePct = Math.floor((host.total_memory_mb * 0.75) / 512) * 512;
  return Math.min(65536, Math.max(8192, seventyFivePct));
}

/**
 * Returns quick-select RAM presets that fit within the host's safe slider limit.
 */
export function getMemoryPresets(host: HostCapabilities | null): number[] {
  const maxMb = getMaxMemoryMb(host);
  const filtered = ALL_MEMORY_PRESETS.filter((p) => p <= maxMb);
  return filtered.length >= 2 ? filtered : [2048, 4096];
}

/**
 * Returns selectable CPU core caps that do not exceed the host's logical cores.
 */
export function getCpuCoreOptions(host: HostCapabilities | null): number[] {
  const maxCores = host && host.cpu_cores > 0 ? host.cpu_cores : 16;
  const options = ALL_CPU_OPTIONS.filter((c) => c <= maxCores);
  if (maxCores > 0 && !options.includes(maxCores) && maxCores <= 32) {
    options.push(maxCores);
    options.sort((a, b) => a - b);
  }
  return options;
}

/**
 * Computes Eco / Balanced / Performance profile values tailored to the host machine.
 */
export function getProfileValues(
  profile: ResourceProfileKind,
  host: HostCapabilities | null,
): ResourceProfileValues {
  if (!host || host.total_memory_mb <= 0) {
    if (profile === "eco") return { memory_mb: 2048, swap_mb: 2048, idle_timeout_secs: 180 };
    if (profile === "balanced") return { memory_mb: 4096, swap_mb: 4096, idle_timeout_secs: 300 };
    return { memory_mb: 8192, swap_mb: 4096, idle_timeout_secs: 900 };
  }

  if (profile === "eco") {
    const mem = Math.min(4096, Math.max(2048, roundToStep(host.total_memory_mb * 0.2, 1024)));
    return { memory_mb: mem, swap_mb: 2048, idle_timeout_secs: 180 };
  }

  if (profile === "balanced") {
    return {
      memory_mb: host.recommended_memory_mb,
      swap_mb: host.recommended_swap_mb,
      idle_timeout_secs: 300,
    };
  }

  const perfMem = Math.min(
    16384,
    Math.max(4096, roundToStep(host.total_memory_mb * 0.5, 1024)),
  );
  return { memory_mb: perfMem, swap_mb: 4096, idle_timeout_secs: 900 };
}

/**
 * Returns true when the configured VM RAM exceeds 70% of the host's physical RAM.
 */
export function isMemoryHighForHost(memoryMb: number, host: HostCapabilities | null): boolean {
  if (!host || host.total_memory_mb <= 0) {
    return false;
  }
  return memoryMb > host.total_memory_mb * 0.7;
}
