import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createEventRefreshScheduler, type RefreshPlan } from "./eventScheduler";

describe("createEventRefreshScheduler", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("coalesces multiple docker_changed events into one selective flush", async () => {
    const flushes: RefreshPlan[] = [];
    const scheduler = createEventRefreshScheduler({
      delayMs: 200,
      isBlocked: () => false,
      onWarning: () => {},
      onFlush: (plan) => {
        flushes.push(plan);
      },
    });

    scheduler.handleEvent({ type: "docker_changed", payload: { resource: "container", action: "create" } });
    scheduler.handleEvent({ type: "docker_changed", payload: { resource: "network", action: "connect" } });
    scheduler.handleEvent({ type: "docker_changed", payload: { resource: "container", action: "start" } });

    expect(flushes).toHaveLength(0);
    await vi.advanceTimersByTimeAsync(200);

    expect(flushes).toEqual([
      {
        status: true,
        containers: true,
        images: false,
        volumes: false,
        networks: true,
      },
    ]);
    scheduler.dispose();
  });

  it("delivers warnings immediately without scheduling a refresh", () => {
    const warnings: string[] = [];
    const flushes: RefreshPlan[] = [];
    const scheduler = createEventRefreshScheduler({
      delayMs: 200,
      isBlocked: () => false,
      onWarning: (msg) => warnings.push(msg),
      onFlush: (plan) => {
        flushes.push(plan);
      },
    });

    scheduler.handleEvent({ type: "warning", payload: { message: "Agent mismatch" } });
    expect(warnings).toEqual(["Agent mismatch"]);
    vi.advanceTimersByTime(500);
    expect(flushes).toHaveLength(0);
    scheduler.dispose();
  });

  it("defers flush while blocked and retries once unblocked", async () => {
    let blocked = true;
    const flushes: RefreshPlan[] = [];
    const scheduler = createEventRefreshScheduler({
      delayMs: 150,
      isBlocked: () => blocked,
      onWarning: () => {},
      onFlush: (plan) => {
        flushes.push(plan);
      },
    });

    scheduler.handleEvent({ type: "docker_changed", payload: { resource: "image", action: "pull" } });
    await vi.advanceTimersByTimeAsync(150);
    expect(flushes).toHaveLength(0);

    blocked = false;
    await vi.advanceTimersByTimeAsync(150);
    expect(flushes).toEqual([
      {
        status: false,
        containers: false,
        images: true,
        volumes: false,
        networks: false,
      },
    ]);
    scheduler.dispose();
  });
});
