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
  loadingResources = $state<boolean>(false);
  autoRefresh = $state<boolean>(true);
  pullingRef = $state<string | null>(null);
  error = $state<string | null>(null);
  notice = $state<string | null>(null);
  theme = $state<ThemeMode>("light");
  private noticeTimer: ReturnType<typeof setTimeout> | null = null;

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
        this.runningContainers = resp.data.running_containers;
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
    await this.refresh();
    if (this.state === "running") {
      await Promise.all([
        this.refreshContainers(silent),
        this.refreshImages(silent),
        this.refreshVolumes(silent),
      ]);
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

  private async runBatch<T>(
    items: T[],
    fn: (item: T) => Promise<ControlResponse>,
    afterRefresh: () => Promise<unknown>,
    successMsg: (succeeded: number) => string,
    errorPrefix: (succeeded: number, total: number, firstError: string) => string,
  ) {
    if (items.length === 0) return;
    this.busy = true;
    this.setNotice(null);
    this.error = null;
    let succeeded = 0;
    const errors: string[] = [];

    try {
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
    } finally {
      this.busy = false;
    }
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
    this.busy = true;
    try {
      const resp = await daemonApi.containerAction(id, action);
      if (!this.handleError(resp)) {
        await Promise.all([this.refreshContainers(), this.refresh()]);
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  async runContainersBatch(ids: string[], action: ContainerActionKind) {
    const verb: Record<ContainerActionKind, string> = {
      start: "Started",
      stop: "Stopped",
      restart: "Restarted",
      remove: "Deleted",
    };
    await this.runBatch(
      ids,
      (id) => daemonApi.containerAction(id, action),
      () => Promise.all([this.refreshContainers(), this.refresh()]),
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
        this.setNotice(`Pulled ${reference}`);
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
    this.busy = true;
    this.setNotice(null);
    try {
      const resp = await daemonApi.runContainer(spec);
      if (resp.status === "container_created") {
        this.setNotice(`Started container ${resp.data.id} from ${spec.image}`);
        await Promise.all([this.refreshContainers(true), this.refreshImages(true), this.refresh()]);
        return true;
      }
      this.handleError(resp);
      return false;
    } catch (e) {
      this.error = String(e);
      return false;
    } finally {
      this.busy = false;
    }
  }

  async removeImage(id: string) {
    this.busy = true;
    try {
      const resp = await daemonApi.removeImage(id);
      if (!this.handleError(resp)) {
        this.setNotice("Image deleted.");
        await this.refreshImages();
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  async removeImagesBatch(ids: string[]) {
    await this.runBatch(
      ids,
      (id) => daemonApi.removeImage(id),
      () => this.refreshImages(),
      (n) => `Deleted ${n} image${n === 1 ? "" : "s"}.`,
      (n, total, err) => `Deleted ${n}/${total} images. Error: ${err}`,
    );
  }

  async pruneImages() {
    this.busy = true;
    this.setNotice(null);
    try {
      const resp = await daemonApi.pruneImages();
      if (resp.status === "prune_result") {
        const mb = (resp.data.reclaimed_bytes / (1024 * 1024)).toFixed(1);
        this.setNotice(`Pruned unused images (${mb} MB reclaimed)`);
        await this.refreshImages();
      } else {
        this.handleError(resp);
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
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
    this.busy = true;
    this.setNotice(null);
    try {
      const resp = await daemonApi.createVolume(name, driver);
      if (!this.handleError(resp)) {
        this.setNotice(`Volume "${name}" created.`);
        await this.refreshVolumes();
        return true;
      }
      return false;
    } catch (e) {
      this.error = String(e);
      return false;
    } finally {
      this.busy = false;
    }
  }

  async removeVolume(name: string) {
    this.busy = true;
    try {
      const resp = await daemonApi.removeVolume(name);
      if (!this.handleError(resp)) {
        this.setNotice(`Volume "${name}" deleted.`);
        await this.refreshVolumes();
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  async removeVolumesBatch(names: string[]) {
    await this.runBatch(
      names,
      (name) => daemonApi.removeVolume(name),
      () => this.refreshVolumes(),
      (n) => `Deleted ${n} volume${n === 1 ? "" : "s"}.`,
      (n, total, err) => `Deleted ${n}/${total} volumes. Error: ${err}`,
    );
  }

  async pruneVolumes() {
    this.busy = true;
    this.setNotice(null);
    try {
      const resp = await daemonApi.pruneVolumes();
      if (resp.status === "prune_result") {
        const mb = (resp.data.reclaimed_bytes / (1024 * 1024)).toFixed(1);
        this.setNotice(`Pruned unused volumes (${mb} MB reclaimed)`);
        await this.refreshVolumes();
      } else {
        this.handleError(resp);
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  async start() {
    this.busy = true;
    try {
      const resp = await daemonApi.startEngine();
      this.handleError(resp);
      await this.refreshAll(true);
    } finally {
      this.busy = false;
    }
  }

  async stop() {
    this.busy = true;
    try {
      const resp = await daemonApi.stopEngine();
      this.handleError(resp);
      await this.refresh();
      if (this.state === "stopped") {
        this.containers = [];
        this.images = [];
        this.volumes = [];
        this.runningContainers = 0;
      }
    } finally {
      this.busy = false;
    }
  }

  async trim() {
    this.busy = true;
    this.setNotice(null);
    try {
      const resp = await daemonApi.trimDisk();
      if (!this.handleError(resp)) {
        this.setNotice("Virtual disk trimmed successfully.");
      }
      await this.refresh();
    } finally {
      this.busy = false;
    }
  }
}

export const engineStore = new EngineStore();
