export type DaemonResource = "container" | "image" | "volume" | "network";

export interface DaemonEventPayload {
  type: string;
  payload?: unknown;
}

export interface RefreshPlan {
  status: boolean;
  containers: boolean;
  images: boolean;
  volumes: boolean;
  networks: boolean;
}

export interface EventSchedulerOptions {
  delayMs?: number;
  isBlocked: () => boolean;
  onWarning: (message: string) => void;
  onFlush: (plan: RefreshPlan) => void | Promise<void>;
}

const EMPTY_PLAN: RefreshPlan = {
  status: false,
  containers: false,
  images: false,
  volumes: false,
  networks: false,
};

/**
 * Coalesces bursts of `nanodock://daemon-event` notifications into a single
 * selective refresh plan, avoiding redundant IPC calls during `compose up` etc.
 */
export function createEventRefreshScheduler(options: EventSchedulerOptions) {
  const delayMs = options.delayMs ?? 200;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let pending: RefreshPlan = { ...EMPTY_PLAN };

  const hasPending = (plan: RefreshPlan) =>
    plan.status || plan.containers || plan.images || plan.volumes || plan.networks;

  const scheduleFlush = () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      timer = null;
      if (options.isBlocked()) {
        if (hasPending(pending)) {
          scheduleFlush();
        }
        return;
      }
      const plan = pending;
      pending = { ...EMPTY_PLAN };
      if (hasPending(plan)) {
        void options.onFlush(plan);
      }
    }, delayMs);
  };

  const handleEvent = (ev: DaemonEventPayload | null | undefined) => {
    if (!ev || typeof ev.type !== "string") return;

    if (ev.type === "warning") {
      if (ev.payload && typeof ev.payload === "object" && "message" in ev.payload) {
        options.onWarning(String((ev.payload as { message: unknown }).message));
      }
      return;
    }

    if (ev.type === "state_changed") {
      pending.status = true;
      if (ev.payload === "running") {
        pending.containers = true;
        pending.images = true;
        pending.volumes = true;
        pending.networks = true;
      }
      scheduleFlush();
      return;
    }

    if (ev.type === "disk_trimmed") {
      pending.status = true;
      scheduleFlush();
      return;
    }

    if (ev.type === "docker_changed") {
      const resource =
        ev.payload && typeof ev.payload === "object" && "resource" in ev.payload
          ? String((ev.payload as { resource: unknown }).resource)
          : "";

      if (resource === "container") {
        pending.status = true;
        pending.containers = true;
      } else if (resource === "image") {
        pending.images = true;
      } else if (resource === "volume") {
        pending.volumes = true;
      } else if (resource === "network") {
        pending.networks = true;
      } else {
        pending.status = true;
        pending.containers = true;
        pending.images = true;
        pending.volumes = true;
        pending.networks = true;
      }
      scheduleFlush();
      return;
    }

    pending.status = true;
    scheduleFlush();
  };

  const dispose = () => {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
    pending = { ...EMPTY_PLAN };
  };

  return { handleEvent, dispose };
}
