import { i18n } from "../i18n/index.svelte";
import {
  daemonApi,
  type ContainerActionKind,
  type ContainerSummary,
  type ControlResponse,
  type EngineConfig,
  type EngineState,
  type ImageSummary,
  type RunContainerSpec,
  type VolumeSummary,
} from "../api/daemon";

export type ThemeMode = "light" | "dark";

class EngineStore {
  state = $state<EngineState>("stopped");
  runningContainers = $state<number>(0);
  vhdxSizeBytes = $state<number | null>(null);
  config = $state<EngineConfig | null>(null);
  containers = $state<ContainerSummary[]>([]);
  images = $state<ImageSummary[]>([]);
  volumes = $state<VolumeSummary[]>([]);
  busy = $state<boolean>(false);
  pendingKeys = $state<Set<string>>(new Set());
  loadingResources = $state<boolean>(false);
  autoRefresh = $state<boolean>(true);
  pullingRef = $state<string | null>(null);
  error = $state<string | null>(null);
  notice = $state<string | null>(null);
  theme = $state<ThemeMode>("light");
  private noticeTimer: ReturnType<typeof setTimeout> | null = null;
  private refreshingAll = false;

  setNotice(msg: string | null) {
    if (this.noticeTimer) {
      clearTimeout(this.noticeTimer);
      this.noticeTimer = null;
    }
    this.notice = msg;
    if (msg) {
      this.noticeTimer = setTimeout(() => {
        if (this.notice === msg) this.notice = null;
      }, 4000);
    }
  }

  initTheme() {
    const savedTheme = localStorage.getItem("nanodock-theme");
    this.theme = savedTheme === "dark" ? "dark" : "light";
    document.documentElement.classList.toggle("dark", this.theme === "dark");

    const savedAutoRefresh = localStorage.getItem("nanodock-auto-refresh");
    if (savedAutoRefresh !== null) {
      this.autoRefresh = savedAutoRefresh === "true";
    }
  }

  toggleTheme() {
    this.theme = this.theme === "light" ? "dark" : "light";
    localStorage.setItem("nanodock-theme", this.theme);
    document.documentElement.classList.toggle("dark", this.theme === "dark");
  }

  toggleAutoRefresh() {
    this.autoRefresh = !this.autoRefresh;
    localStorage.setItem("nanodock-auto-refresh", String(this.autoRefresh));
  }

  private handleError(resp: ControlResponse): boolean {
    if (resp.status === "error") {
      this.error = resp.data.message;
      return true;
    }
    return false;
  }

  async refresh() {
    try {
      const resp = await daemonApi.getStatus();
      if (resp.status === "status") {
        this.state = resp.data.state;
        if (this.state === "stopped") {
          this.containers = [];
          this.images = [];
          this.volumes = [];
          this.runningContainers = 0;
        } else if (this.containers.length === 0) {
          this.runningContainers = resp.data.running_containers;
        }
        this.vhdxSizeBytes = resp.data.vhdx_size_bytes;
        this.error = null;
      } else {
        this.handleError(resp);
      }
    } catch (e) {
      this.error = String(e);
    }
  }

  async refreshAll(silent = false) {
    if (this.refreshingAll) return;
    this.refreshingAll = true;
    try {
      await this.refresh();
      if (this.state === "running") {
        await Promise.all([
          this.refreshContainers(silent),
          this.refreshImages(silent),
          this.refreshVolumes(silent),
        ]);
      }
    } finally {
      this.refreshingAll = false;
    }
  }

  async loadConfig() {
    try {
      const resp = await daemonApi.getConfig();
      if (resp.status === "config") {
        this.config = resp.data;
        this.error = null;
      } else {
        this.handleError(resp);
      }
    } catch (e) {
      this.error = String(e);
    }
  }

  isPending(key: string): boolean {
    return this.pendingKeys.has(key);
  }

  isPrefixPending(prefix: string): boolean {
    for (const k of this.pendingKeys) {
      if (k.startsWith(prefix)) return true;
    }
    return false;
  }

  private async withPending<T>(key: string, fn: () => Promise<T>): Promise<T> {
    const next = new Set(this.pendingKeys);
    next.add(key);
    this.pendingKeys = next;
    this.busy = true;
    try {
      return await fn();
    } finally {
      const after = new Set(this.pendingKeys);
      after.delete(key);
      this.pendingKeys = after;
      this.busy = after.size > 0;
    }
  }

  private async runBatch<T>(
    key: string,
    items: T[],
    fn: (item: T) => Promise<ControlResponse>,
    afterRefresh: () => Promise<unknown>,
    successMsg: (succeeded: number) => string,
    errorPrefix: (succeeded: number, total: number, firstError: string) => string,
  ) {
    if (items.length === 0) return;
    await this.withPending(key, async () => {
      this.setNotice(null);
      this.error = null;
      let succeeded = 0;
      const errors: string[] = [];

      const results = await Promise.allSettled(items.map((item) => fn(item)));
      for (const res of results) {
        if (res.status === "fulfilled") {
          if (res.value.status === "error") {
            errors.push(res.value.data.message);
          } else {
            succeeded += 1;
          }
        } else {
          errors.push(String(res.reason));
        }
      }

      await afterRefresh();

      if (errors.length > 0) {
        this.error = errorPrefix(succeeded, items.length, errors[0]);
      } else {
        this.setNotice(successMsg(succeeded));
      }
    });
  }

  async refreshContainers(silent = false) {
    if (this.state === "stopped") return;
    if (!silent) this.loadingResources = true;
    try {
      const resp = await daemonApi.listContainers();
      if (resp.status === "containers") {
        this.containers = resp.data;
        this.runningContainers = resp.data.filter((c) => c.state === "running").length;
        this.error = null;
      } else {
        this.handleError(resp);
      }
    } catch (e) {
      if (!silent) this.error = String(e);
    } finally {
      if (!silent) this.loadingResources = false;
    }
  }

  async runContainerAction(id: string, action: ContainerActionKind) {
    await this.withPending(`container:${id}:${action}`, async () => {
      try {
        const resp = await daemonApi.containerAction(id, action);
        if (!this.handleError(resp)) {
          this.setNotice(i18n.t.notices.containerActionDone(action));
          await Promise.all([this.refreshContainers(true), this.refresh()]);
        }
      } catch (e) {
        this.error = String(e);
      }
    });
  }

  async runContainersBatch(ids: string[], action: ContainerActionKind, scopeKey = "containers:batch") {
    const verb: Record<ContainerActionKind, string> = {
      start: "Started",
      stop: "Stopped",
      restart: "Restarted",
      remove: "Deleted",
    };
    await this.runBatch(
      `${scopeKey}:${action}`,
      ids,
      (id) => daemonApi.containerAction(id, action),
      () => Promise.all([this.refreshContainers(true), this.refresh()]),
      (n) => `${verb[action]} ${n} container${n === 1 ? "" : "s"}.`,
      (n, total, err) => `${n}/${total} succeeded. Error: ${err}`,
    );
  }

  async refreshImages(silent = false) {
    if (this.state === "stopped") return;
    if (!silent) this.loadingResources = true;
    try {
      const resp = await daemonApi.listImages();
      if (resp.status === "images") {
        this.images = resp.data;
        this.error = null;
      } else {
        this.handleError(resp);
      }
    } catch (e) {
      if (!silent) this.error = String(e);
    } finally {
      if (!silent) this.loadingResources = false;
    }
  }

  async pullImage(reference: string): Promise<boolean> {
    if (this.pullingRef) return false;
    this.pullingRef = reference;
    this.setNotice(null);
    try {
      const resp = await daemonApi.pullImage(reference);
      if (!this.handleError(resp)) {
        this.setNotice(i18n.t.notices.pulledImage(reference));
        await this.refreshImages(true);
        return true;
      }
      return false;
    } catch (e) {
      this.error = String(e);
      return false;
    } finally {
      this.pullingRef = null;
    }
  }

  async runContainer(spec: RunContainerSpec): Promise<boolean> {
    return this.withPending("container:run", async () => {
      this.setNotice(null);
      try {
        const resp = await daemonApi.runContainer(spec);
        if (resp.status === "container_created") {
          this.setNotice(i18n.t.notices.startedContainer(resp.data.id, spec.image));
          await Promise.all([this.refreshContainers(true), this.refreshImages(true), this.refresh()]);
          return true;
        }
        this.handleError(resp);
        return false;
      } catch (e) {
        this.error = String(e);
        return false;
      }
    });
  }

  async removeImage(id: string) {
    await this.withPending(`image:${id}:remove`, async () => {
      try {
        const resp = await daemonApi.removeImage(id);
        if (!this.handleError(resp)) {
          this.setNotice(i18n.t.notices.imageDeleted);
          await this.refreshImages(true);
        }
      } catch (e) {
        this.error = String(e);
      }
    });
  }

  async removeImagesBatch(ids: string[]) {
    await this.runBatch(
      "images:batch:remove",
      ids,
      (id) => daemonApi.removeImage(id),
      () => this.refreshImages(true),
      (n) => i18n.t.notices.imagesDeletedBatch(n),
      (n, total, err) => i18n.t.notices.imagesDeletedBatchError(n, total, err),
    );
  }

  async pruneImages() {
    await this.withPending("images:prune", async () => {
      this.setNotice(null);
      try {
        const resp = await daemonApi.pruneImages();
        if (resp.status === "prune_result") {
          const mb = (resp.data.reclaimed_bytes / (1024 * 1024)).toFixed(1);
          this.setNotice(i18n.t.notices.imagesPruned(mb));
          await this.refreshImages(true);
        } else {
          this.handleError(resp);
        }
      } catch (e) {
        this.error = String(e);
      }
    });
  }

  async refreshVolumes(silent = false) {
    if (this.state === "stopped") return;
    if (!silent) this.loadingResources = true;
    try {
      const resp = await daemonApi.listVolumes();
      if (resp.status === "volumes") {
        this.volumes = resp.data;
        this.error = null;
      } else {
        this.handleError(resp);
      }
    } catch (e) {
      if (!silent) this.error = String(e);
    } finally {
      if (!silent) this.loadingResources = false;
    }
  }

  async createVolume(name: string, driver?: string): Promise<boolean> {
    return this.withPending("volumes:create", async () => {
      this.setNotice(null);
      try {
        const resp = await daemonApi.createVolume(name, driver);
        if (!this.handleError(resp)) {
          this.setNotice(i18n.t.notices.volumeCreated(name));
          await this.refreshVolumes(true);
          return true;
        }
        return false;
      } catch (e) {
        this.error = String(e);
        return false;
      }
    });
  }

  async removeVolume(name: string) {
    await this.withPending(`volume:${name}:remove`, async () => {
      try {
        const resp = await daemonApi.removeVolume(name);
        if (!this.handleError(resp)) {
          this.setNotice(i18n.t.notices.volumeDeleted(name));
          await this.refreshVolumes(true);
        }
      } catch (e) {
        this.error = String(e);
      }
    });
  }

  async removeVolumesBatch(names: string[]) {
    await this.runBatch(
      "volumes:batch:remove",
      names,
      (name) => daemonApi.removeVolume(name),
      () => this.refreshVolumes(true),
      (n) => i18n.t.notices.volumesDeletedBatch(n),
      (n, total, err) => i18n.t.notices.volumesDeletedBatchError(n, total, err),
    );
  }

  async pruneVolumes() {
    await this.withPending("volumes:prune", async () => {
      this.setNotice(null);
      try {
        const resp = await daemonApi.pruneVolumes();
        if (resp.status === "prune_result") {
          const mb = (resp.data.reclaimed_bytes / (1024 * 1024)).toFixed(1);
          this.setNotice(i18n.t.notices.volumesPruned(mb));
          await this.refreshVolumes(true);
        } else {
          this.handleError(resp);
        }
      } catch (e) {
        this.error = String(e);
      }
    });
  }

  async start() {
    await this.withPending("engine:start", async () => {
      const resp = await daemonApi.startEngine();
      if (!this.handleError(resp)) {
        this.setNotice(i18n.t.notices.engineStarted);
      }
      await this.refreshAll(true);
    });
  }

  async stop() {
    await this.withPending("engine:stop", async () => {
      const resp = await daemonApi.stopEngine();
      if (!this.handleError(resp)) {
        this.setNotice(i18n.t.notices.engineStopped);
      }
      await this.refresh();
      if (this.state === "stopped") {
        this.containers = [];
        this.images = [];
        this.volumes = [];
        this.runningContainers = 0;
      }
    });
  }

  async trim() {
    await this.withPending("engine:trim", async () => {
      this.setNotice(null);
      try {
        const resp = await daemonApi.trimDisk();
        if (!this.handleError(resp)) {
          this.setNotice(i18n.t.notices.diskTrimmed);
        }
        await this.refresh();
      } catch (e) {
        this.error = String(e);
      }
    });
  }
}

export const engineStore = new EngineStore();
