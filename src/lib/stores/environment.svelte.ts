import { systemApi, type EnvironmentStatus, type HostCapabilities } from "../api/daemon";
import { i18n } from "../i18n/index.svelte";
import { engineStore } from "./engine.svelte";

class EnvironmentStore {
  status = $state<EnvironmentStatus | null>(null);
  host = $state<HostCapabilities | null>(null);
  loading = $state(false);
  busyAction = $state<string | null>(null);
  error = $state<string | null>(null);
  bannerDismissed = $state(false);

  async refresh() {
    this.loading = true;
    this.error = null;
    try {
      const [status, host] = await Promise.all([
        systemApi.getEnvironmentStatus(),
        systemApi.getHostCapabilities().catch(() => null),
      ]);
      this.status = status;
      this.host = host;
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loading = false;
    }
  }

  async installWsl2() {
    this.busyAction = "wsl2";
    this.error = null;
    try {
      await systemApi.installWsl2();
      engineStore.setNotice(i18n.t.notices.wslInstallStarted);
      await this.refresh();
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busyAction = null;
    }
  }

  async setUserPath(enabled: boolean) {
    this.busyAction = "path";
    this.error = null;
    try {
      this.status = await systemApi.setUserPathIntegration(enabled);
      engineStore.setNotice(enabled ? i18n.t.notices.pathEnabled : i18n.t.notices.pathDisabled);
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busyAction = null;
    }
  }

  async setDockerShim(enabled: boolean) {
    this.busyAction = "shim";
    this.error = null;
    try {
      this.status = await systemApi.setDockerCliShim(enabled);
      engineStore.setNotice(
        enabled ? i18n.t.notices.dockerShimEnabled : i18n.t.notices.dockerShimDisabled,
      );
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busyAction = null;
    }
  }

  async fixCredsStore() {
    this.busyAction = "creds";
    this.error = null;
    try {
      this.status = await systemApi.fixDockerCredsStore();
      engineStore.setNotice(i18n.t.notices.credsStoreFixed);
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busyAction = null;
    }
  }

  dismissBanner() {
    this.bannerDismissed = true;
  }
}

export const environmentStore = new EnvironmentStore();
