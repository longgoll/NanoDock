import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

class UpdaterStore {
  checking = $state(false);
  downloading = $state(false);
  installing = $state(false);
  progress = $state<number | null>(null);
  availableVersion = $state<string | null>(null);
  releaseNotes = $state<string | null>(null);
  error = $state<string | null>(null);
  upToDateChecked = $state(false);
  dismissed = $state(false);

  private pendingUpdate: Update | null = null;

  async checkForUpdates(silent = false): Promise<void> {
    if (this.checking || this.downloading || this.installing) return;
    this.checking = true;
    this.error = null;
    if (!silent) {
      this.upToDateChecked = false;
    }

    try {
      const update = await check();
      if (this.pendingUpdate && this.pendingUpdate !== update) {
        void this.pendingUpdate.close().catch(() => {});
      }

      if (update) {
        this.pendingUpdate = update;
        this.availableVersion = update.version;
        this.releaseNotes = update.body ?? null;
        this.dismissed = false;
        this.upToDateChecked = false;
      } else {
        this.pendingUpdate = null;
        this.availableVersion = null;
        this.releaseNotes = null;
        if (!silent) {
          this.upToDateChecked = true;
        }
      }
    } catch (e) {
      if (!silent) {
        this.error = e instanceof Error ? e.message : String(e);
      }
    } finally {
      this.checking = false;
    }
  }

  async installUpdate(): Promise<void> {
    if (!this.pendingUpdate || this.downloading || this.installing) return;
    this.downloading = true;
    this.installing = false;
    this.progress = 0;
    this.error = null;

    try {
      let totalBytes = 0;
      let downloadedBytes = 0;

      await this.pendingUpdate.download((event) => {
        if (event.event === "Started") {
          totalBytes = event.data.contentLength ?? 0;
          downloadedBytes = 0;
          this.progress = totalBytes > 0 ? 0 : null;
        } else if (event.event === "Progress") {
          downloadedBytes += event.data.chunkLength;
          if (totalBytes > 0) {
            this.progress = Math.min(100, Math.round((downloadedBytes / totalBytes) * 100));
          }
        } else if (event.event === "Finished") {
          this.progress = 100;
        }
      });

      this.downloading = false;
      this.installing = true;

      // Shut down core-daemon.exe and allow app exit so NSIS can overwrite files cleanly.
      await invoke("prepare_for_update");
      await this.pendingUpdate.install();
      await relaunch();
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      this.downloading = false;
      this.installing = false;
      this.progress = null;
    }
  }

  dismissBanner(): void {
    this.dismissed = true;
  }
}

export const updaterStore = new UpdaterStore();
