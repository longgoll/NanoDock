import {
  daemonApi,
  type ContainerActionKind,
  type ContainerSummary,
  type ControlResponse,
  type EngineConfig,
  type EngineState,
  type ImageSummary,
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
  error = $state<string | null>(null);
  notice = $state<string | null>(null);
  theme = $state<ThemeMode>("light");

  initTheme() {
    const saved = localStorage.getItem("nanodock-theme");
    this.theme = saved === "dark" ? "dark" : "light";
    document.documentElement.classList.toggle("dark", this.theme === "dark");
  }

  toggleTheme() {
    this.theme = this.theme === "light" ? "dark" : "light";
    localStorage.setItem("nanodock-theme", this.theme);
    document.documentElement.classList.toggle("dark", this.theme === "dark");
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

  async refreshContainers() {
    this.loadingResources = true;
    try {
      const resp = await daemonApi.listContainers();
      if (resp.status === "containers") {
        this.containers = resp.data;
        this.error = null;
      } else {
        this.handleError(resp);
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loadingResources = false;
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

  async refreshImages() {
    this.loadingResources = true;
    try {
      const resp = await daemonApi.listImages();
      if (resp.status === "images") {
        this.images = resp.data;
        this.error = null;
      } else {
        this.handleError(resp);
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loadingResources = false;
    }
  }

  async pullImage(reference: string) {
    this.busy = true;
    this.notice = null;
    try {
      const resp = await daemonApi.pullImage(reference);
      if (!this.handleError(resp)) {
        this.notice = `Pulled ${reference}`;
        await this.refreshImages();
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  async removeImage(id: string) {
    this.busy = true;
    try {
      const resp = await daemonApi.removeImage(id);
      if (!this.handleError(resp)) {
        await this.refreshImages();
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  async pruneImages() {
    this.busy = true;
    this.notice = null;
    try {
      const resp = await daemonApi.pruneImages();
      if (resp.status === "prune_result") {
        const mb = (resp.data.reclaimed_bytes / (1024 * 1024)).toFixed(1);
        this.notice = `Pruned unused images (${mb} MB reclaimed)`;
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

  async refreshVolumes() {
    this.loadingResources = true;
    try {
      const resp = await daemonApi.listVolumes();
      if (resp.status === "volumes") {
        this.volumes = resp.data;
        this.error = null;
      } else {
        this.handleError(resp);
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loadingResources = false;
    }
  }

  async removeVolume(name: string) {
    this.busy = true;
    try {
      const resp = await daemonApi.removeVolume(name);
      if (!this.handleError(resp)) {
        await this.refreshVolumes();
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  async pruneVolumes() {
    this.busy = true;
    this.notice = null;
    try {
      const resp = await daemonApi.pruneVolumes();
      if (resp.status === "prune_result") {
        const mb = (resp.data.reclaimed_bytes / (1024 * 1024)).toFixed(1);
        this.notice = `Pruned unused volumes (${mb} MB reclaimed)`;
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
      await this.refresh();
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
    } finally {
      this.busy = false;
    }
  }

  async trim() {
    this.busy = true;
    this.notice = null;
    try {
      const resp = await daemonApi.trimDisk();
      if (!this.handleError(resp)) {
        this.notice = "Virtual disk trimmed successfully.";
      }
      await this.refresh();
    } finally {
      this.busy = false;
    }
  }
}

export const engineStore = new EngineStore();
