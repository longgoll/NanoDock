<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { onDestroy, onMount } from "svelte";
  import { daemonApi, systemApi, type EngineConfig, type WslDistroInfo } from "../../lib/api/daemon";
  import HelpTip from "../../lib/components/HelpTip.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import RefreshButton from "../../lib/components/RefreshButton.svelte";
  import { i18n, type LocalePreference } from "../../lib/i18n/index.svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";
  import { environmentStore } from "../../lib/stores/environment.svelte";
  import { updaterStore } from "../../lib/stores/updater.svelte";
  import DiagnosticsModal from "./DiagnosticsModal.svelte";

  const DEFAULT_CONFIG: EngineConfig = {
    distro_name: "nanodock",
    idle_timeout_secs: 180,
    auto_start_on_login: false,
    auto_trim_on_events: true,
    wsl: {
      memory_mb: 4096,
      swap_mb: 4096,
      sparse_vhd: true,
      mirrored_networking: true,
    },
  };

  type SettingsTab = "general" | "engine" | "system" | "updates";
  let activeSettingsTab = $state<SettingsTab>("engine");

  let saving = $state(false);
  let savingRestart = $state(false);
  let loading = $state(false);
  let appVersion = $state<string | null>(null);
  let draft = $state<EngineConfig>(cloneConfig(DEFAULT_CONFIG));
  let wslDistros = $state<WslDistroInfo[]>([]);
  let distroBusy = $state<string | null>(null);
  let diagnosticsOpen = $state(false);

  async function refreshWslDistros() {
    try {
      const res = await daemonApi.listWslDistros();
      if (res.status === "wsl_distros") {
        wslDistros = res.data;
      }
    } catch {
      // ignore when daemon is offline
    }
  }

  async function toggleDistroIntegration(distro: WslDistroInfo) {
    distroBusy = distro.name;
    try {
      const res = await daemonApi.setWslDistroIntegration(distro.name, !distro.integrated);
      if (res.status === "ack") {
        await refreshWslDistros();
      }
    } finally {
      distroBusy = null;
    }
  }

  function applyResourceProfile(profile: "eco" | "balanced" | "perf") {
    if (profile === "eco") {
      draft.wsl.memory_mb = 2048;
      draft.wsl.swap_mb = 2048;
      draft.idle_timeout_secs = 180;
    } else if (profile === "balanced") {
      draft.wsl.memory_mb = 4096;
      draft.wsl.swap_mb = 4096;
      draft.idle_timeout_secs = 300;
    } else {
      draft.wsl.memory_mb = 8192;
      draft.wsl.swap_mb = 4096;
      draft.idle_timeout_secs = 900;
    }
  }

  const activeProfile = $derived.by(() => {
    if (draft.wsl.memory_mb === 2048 && draft.wsl.swap_mb === 2048 && draft.idle_timeout_secs === 180) {
      return "eco";
    }
    if (draft.wsl.memory_mb === 4096 && draft.wsl.swap_mb === 4096 && draft.idle_timeout_secs === 300) {
      return "balanced";
    }
    if (draft.wsl.memory_mb === 8192 && draft.wsl.swap_mb === 4096 && draft.idle_timeout_secs === 900) {
      return "perf";
    }
    return null;
  });

  const idleOptions = $derived<{ value: number | null; label: string }[]>([
    { value: null, label: i18n.t.settings.idleNever },
    { value: 180, label: i18n.t.settings.idle3m },
    { value: 300, label: i18n.t.settings.idle5m },
    { value: 900, label: i18n.t.settings.idle15m },
    { value: 1800, label: i18n.t.settings.idle30m },
    { value: 3600, label: i18n.t.settings.idle60m },
  ]);

  const memoryPresets = [2048, 4096, 8192, 12288, 16384];

  function cloneConfig(cfg: EngineConfig): EngineConfig {
    return {
      ...cfg,
      wsl: { ...cfg.wsl },
    };
  }

  async function load() {
    loading = true;
    try {
      await engineStore.loadConfig();
      if (engineStore.config) {
        draft = cloneConfig(engineStore.config);
      }
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    void getVersion()
      .then((v) => {
        appVersion = v;
      })
      .catch(() => {
        appVersion = null;
      });
    if (engineStore.config) {
      draft = cloneConfig(engineStore.config);
    }
    void load();
    void environmentStore.refresh();
    void refreshWslDistros();
  });

  const isConnected = $derived(engineStore.config !== null);

  const isDirty = $derived.by(() => {
    if (!engineStore.config) return false;
    return JSON.stringify(draft) !== JSON.stringify(engineStore.config);
  });

  $effect(() => {
    engineStore.settingsDirty = isDirty;
  });

  onDestroy(() => {
    engineStore.settingsDirty = false;
  });

  const isDefault = $derived(
    JSON.stringify(draft) === JSON.stringify(DEFAULT_CONFIG),
  );

  const wslDirty = $derived.by(() => {
    if (!engineStore.config) return false;
    return JSON.stringify(draft.wsl) !== JSON.stringify(engineStore.config.wsl);
  });

  const hasCustomIdle = $derived(
    !idleOptions.some((o) => o.value === draft.idle_timeout_secs),
  );

  function formatGb(mb: number): string {
    return `${(mb / 1024).toFixed(mb % 1024 === 0 ? 0 : 1)} GB`;
  }

  function resetDraft() {
    if (engineStore.config) {
      draft = cloneConfig(engineStore.config);
    }
  }

  function restoreDefaults() {
    draft = {
      ...cloneConfig(DEFAULT_CONFIG),
      distro_name: engineStore.config?.distro_name ?? DEFAULT_CONFIG.distro_name,
    };
  }

  async function save(restartAfter = false) {
    if (!isConnected) return;
    saving = true;
    savingRestart = restartAfter;
    try {
      const resp = await daemonApi.updateConfig(draft);
      if (resp.status === "ack") {
        const wslChanged = wslDirty;
        engineStore.config = cloneConfig(draft);
        if (restartAfter && engineStore.state !== "stopped") {
          await engineStore.stop();
          await engineStore.start();
          engineStore.setNotice(i18n.t.notices.settingsSavedRestarted);
        } else if (wslChanged && engineStore.state === "running") {
          engineStore.setNotice(i18n.t.notices.settingsSavedWslRestart);
        } else {
          engineStore.setNotice(i18n.t.notices.settingsSaved);
        }
      } else if (resp.status === "error") {
        engineStore.error = resp.data.message;
      }
    } catch (e) {
      engineStore.error = String(e);
    } finally {
      saving = false;
      savingRestart = false;
    }
  }
</script>

<section class="space-y-6">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <h2 class="text-lg font-semibold text-slate-900 dark:text-white">
        {i18n.t.settings.title}
      </h2>
      <p class="text-xs text-slate-500 dark:text-slate-400">
        {i18n.t.settings.subtitle}
      </p>
    </div>

    <div class="flex items-center gap-2">
      {#if isDirty}
        <span class="rounded-full border border-amber-200 bg-amber-50 px-2.5 py-1 text-xs font-medium text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/15 dark:text-amber-300">
          {i18n.t.settings.unsavedChanges}
        </span>
      {/if}
      <RefreshButton
        onRefresh={load}
        disabled={loading || saving}
      />
    </div>
  </div>

  <!-- Settings Sub-navigation Tabs -->
  <div class="flex flex-wrap items-center gap-1 border-b border-slate-200 pb-2 dark:border-slate-800">
    {#each [
      { id: "engine", label: i18n.t.settings.tabEngine, icon: "settings" },
      { id: "general", label: i18n.t.settings.tabGeneral, icon: "dashboard" },
      { id: "system", label: i18n.t.settings.tabSystem, icon: "terminal" },
      { id: "updates", label: i18n.t.settings.tabUpdates, icon: "refresh" },
    ] as const as tab}
      <button
        type="button"
        onclick={() => (activeSettingsTab = tab.id)}
        class={`inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition ${
          activeSettingsTab === tab.id
            ? "bg-sky-50 text-sky-700 dark:bg-sky-500/15 dark:text-sky-300"
            : "text-slate-600 hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
        }`}
      >
        <Icon name={tab.icon} class="h-3.5 w-3.5" />
        {tab.label}
      </button>
    {/each}
  </div>

  {#if activeSettingsTab === "general"}
  <!-- Language Preference -->
  <div class="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
    <div>
      <h3 class="text-sm font-semibold text-slate-900 dark:text-white">
        {i18n.t.settings.languageTitle}
      </h3>
      <p class="text-xs text-slate-400">
        {i18n.t.settings.languageDesc}
      </p>
    </div>
    <div class="inline-flex rounded-lg border border-slate-200 bg-slate-50 p-0.5 dark:border-slate-700 dark:bg-slate-950">
      {#each [
        { id: "auto", label: i18n.t.settings.languageAuto(i18n.systemLocale === "vi" ? "Tiếng Việt" : "English") },
        { id: "en", label: i18n.t.settings.languageEn },
        { id: "vi", label: i18n.t.settings.languageVi },
      ] as opt}
        <button
          type="button"
          onclick={() => i18n.setPreference(opt.id as LocalePreference)}
          class={`rounded-md px-3 py-1.5 text-xs font-medium transition ${
            i18n.preference === opt.id
              ? "bg-sky-600 text-white shadow-xs"
              : "text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200"
          }`}
        >
          {opt.label}
        </button>
      {/each}
    </div>
  </div>

  <!-- Background Auto-Sync -->
  <div class="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
    <div>
      <h3 class="text-sm font-semibold text-slate-900 dark:text-white">
        {i18n.t.settings.autoSyncLabel}
      </h3>
      <p class="text-xs text-slate-400">
        {i18n.t.settings.autoSyncDesc}
      </p>
    </div>
    <input
      type="checkbox"
      checked={engineStore.autoRefresh}
      onchange={() => engineStore.toggleAutoRefresh()}
      aria-label={i18n.t.settings.autoSyncLabel}
      class="h-4 w-4 shrink-0 accent-sky-600"
    />
  </div>
  {/if}

  {#if activeSettingsTab === "updates"}
  <!-- Application Updates -->
  <div class="space-y-3 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
    <div class="flex flex-wrap items-center justify-between gap-4">
      <div>
        <div class="flex items-center gap-2">
          <h3 class="text-sm font-semibold text-slate-900 dark:text-white">
            {i18n.t.settings.updateTitle}
          </h3>
          {#if appVersion}
            <span class="rounded-md bg-slate-100 px-2 py-0.5 font-mono text-[11px] font-medium text-slate-600 dark:bg-slate-800 dark:text-slate-300">
              v{appVersion}
            </span>
          {/if}
        </div>
        <p class="text-xs text-slate-400">
          {i18n.t.settings.updateDesc}
        </p>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onclick={() => void updaterStore.checkForUpdates(false)}
          disabled={updaterStore.checking || updaterStore.downloading || updaterStore.installing}
          class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
        >
          {#if updaterStore.checking}
            <Icon name="spinner" class="h-3.5 w-3.5 animate-spin" />
          {/if}
          {updaterStore.checking ? i18n.t.settings.checkingUpdates : i18n.t.settings.checkForUpdates}
        </button>

        {#if updaterStore.availableVersion}
          <button
            type="button"
            onclick={() => void updaterStore.installUpdate()}
            disabled={updaterStore.downloading || updaterStore.installing}
            class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3 py-1.5 text-xs font-medium text-white shadow-xs transition hover:bg-sky-500 disabled:opacity-60"
          >
            {#if updaterStore.downloading || updaterStore.installing}
              <Icon name="spinner" class="h-3.5 w-3.5 animate-spin" />
            {/if}
            {updaterStore.installing
              ? i18n.t.settings.installingUpdate
              : updaterStore.downloading
                ? i18n.t.settings.downloadingUpdate(updaterStore.progress)
                : i18n.t.settings.updateNow}
          </button>
        {/if}
      </div>
    </div>

    {#if updaterStore.availableVersion}
      <div class="rounded-xl border border-sky-200/80 bg-sky-50/70 px-3.5 py-2.5 text-xs text-sky-900 dark:border-sky-500/25 dark:bg-sky-500/10 dark:text-sky-200">
        <p class="font-medium">{i18n.t.settings.updateAvailable(updaterStore.availableVersion)}</p>
        {#if updaterStore.releaseNotes}
          <p class="mt-1 whitespace-pre-line text-[11px] text-sky-800/90 dark:text-sky-300/90">
            {updaterStore.releaseNotes}
          </p>
        {/if}
        {#if updaterStore.downloading && updaterStore.progress !== null}
          <div class="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-sky-200 dark:bg-sky-900">
            <div
              class="h-full bg-sky-600 transition-all duration-150 dark:bg-sky-400"
              style={`width: ${updaterStore.progress}%`}
            ></div>
          </div>
        {/if}
      </div>
    {:else if updaterStore.upToDateChecked}
      <p class="text-xs text-emerald-600 dark:text-emerald-400">
        {i18n.t.settings.upToDate}
      </p>
    {/if}

    {#if updaterStore.error}
      <p class="text-xs text-rose-600 dark:text-rose-400">
        {updaterStore.error}
      </p>
    {/if}
  </div>
  {/if}

  {#if activeSettingsTab === "system"}
  <!-- System & CLI Integration -->
  <div class="space-y-4 rounded-2xl border border-slate-200 bg-white p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
    <div class="flex flex-wrap items-center justify-between gap-3 border-b border-slate-200 pb-4 dark:border-slate-800">
      <div>
        <h3 class="text-sm font-semibold text-slate-900 dark:text-white">{i18n.t.settings.systemTitle}</h3>
        <p class="text-xs text-slate-400">{i18n.t.settings.systemDesc}</p>
      </div>
      {#if environmentStore.status}
        <span
          class={`rounded-full px-2.5 py-1 text-xs font-medium ${
            environmentStore.status.wsl_installed
              ? "bg-emerald-50 text-emerald-700 dark:bg-emerald-500/15 dark:text-emerald-300"
              : "bg-amber-50 text-amber-700 dark:bg-amber-500/15 dark:text-amber-300"
          }`}
        >
          WSL2: {environmentStore.status.wsl_installed ? i18n.t.settings.wslInstalled : i18n.t.settings.wslMissing}
        </span>
      {/if}
    </div>

    {#if environmentStore.status}
      {@const env = environmentStore.status}
      <div class="space-y-4">
        {#if !env.wsl_installed}
          <div class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-amber-200 bg-amber-50/80 px-4 py-3 text-xs text-amber-900 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-200">
            <div>
              <p class="font-medium">{i18n.t.settings.wslStatusLabel}: {i18n.t.settings.wslMissing}</p>
              {#if env.wsl_status_detail}
                <p class="mt-0.5 text-[11px] opacity-80">{env.wsl_status_detail}</p>
              {/if}
            </div>
            <button
              type="button"
              onclick={() => void environmentStore.installWsl2()}
              disabled={environmentStore.busyAction !== null}
              class="inline-flex items-center gap-1.5 rounded-lg bg-amber-600 px-3 py-1.5 font-medium text-white shadow-xs transition hover:bg-amber-500 disabled:opacity-50"
            >
              {#if environmentStore.busyAction === "wsl2"}
                <Icon name="spinner" class="h-3.5 w-3.5 animate-spin" />
              {/if}
              {environmentStore.busyAction === "wsl2" ? i18n.t.settings.installingWsl2 : i18n.t.settings.installWsl2}
            </button>
          </div>
        {/if}

        {#if env.creds_store_warning}
          <div class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-amber-200 bg-amber-50/80 px-4 py-3 text-xs text-amber-900 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-200">
            <div>
              <p class="font-medium">{i18n.t.settings.credsWarningTitle}</p>
              <p class="mt-0.5 text-[11px] opacity-80">{env.creds_store_warning}</p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
              <button
                type="button"
                onclick={() => void environmentStore.fixCredsStore()}
                disabled={environmentStore.busyAction !== null}
                class="inline-flex items-center gap-1.5 rounded-lg bg-amber-600 px-3 py-1.5 font-medium text-white shadow-xs transition hover:bg-amber-500 disabled:opacity-50"
              >
                {#if environmentStore.busyAction === "creds"}
                  <Icon name="spinner" class="h-3.5 w-3.5 animate-spin" />
                {/if}
                {i18n.t.settings.credsWarningFix}
              </button>
              {#if env.wincred_available}
                <button
                  type="button"
                  onclick={async () => {
                    await systemApi.setWincredCredsStore();
                    await environmentStore.refresh();
                  }}
                  disabled={environmentStore.busyAction !== null}
                  class="inline-flex items-center gap-1.5 rounded-lg border border-amber-500/40 bg-amber-500/10 px-3 py-1.5 font-medium text-amber-800 transition hover:bg-amber-500/20 disabled:opacity-50 dark:text-amber-200"
                >
                  {i18n.t.extra.useWincred}
                </button>
              {/if}
            </div>
          </div>
        {/if}

        <div class="flex items-start justify-between gap-4">
          <div>
            <p class="text-sm font-medium text-slate-700 dark:text-slate-300">
              {i18n.t.settings.userPathLabel}
            </p>
            <p class="mt-0.5 text-xs text-slate-400">
              {i18n.t.settings.userPathDesc}
            </p>
            <p class="mt-1 font-mono text-[11px] text-slate-400">{env.install_dir}</p>
          </div>
          <input
            type="checkbox"
            checked={env.nanodock_in_path}
            disabled={environmentStore.busyAction !== null}
            onchange={(e) => void environmentStore.setUserPath(e.currentTarget.checked)}
            class="mt-1 h-4 w-4 shrink-0 accent-sky-600 disabled:opacity-50"
          />
        </div>

        <div class="flex items-start justify-between gap-4 border-t border-slate-200 pt-4 dark:border-slate-800">
          <div>
            <p class="text-sm font-medium text-slate-700 dark:text-slate-300">
              {i18n.t.settings.dockerShimLabel}
            </p>
            <p class="mt-0.5 text-xs text-slate-400">
              {i18n.t.settings.dockerShimDesc}
            </p>
            {#if !env.nanodock_cli_available}
              <p class="mt-1 text-[11px] text-amber-600 dark:text-amber-400">
                {i18n.t.settings.cliUnavailable}
              </p>
            {:else if env.external_docker_path}
              <p class="mt-1 font-mono text-[11px] text-slate-400">
                {i18n.t.settings.dockerShimExternalFound(env.external_docker_path)}
              </p>
            {/if}
          </div>
          <input
            type="checkbox"
            checked={env.docker_shim_active}
            disabled={!env.nanodock_cli_available || environmentStore.busyAction !== null}
            onchange={(e) => void environmentStore.setDockerShim(e.currentTarget.checked)}
            class="mt-1 h-4 w-4 shrink-0 accent-sky-600 disabled:opacity-50"
          />
        </div>
      </div>
    {:else if environmentStore.loading}
      <p class="text-xs text-slate-400">{i18n.t.settings.checkingUpdates}</p>
    {/if}

    {#if environmentStore.error}
      <p class="text-xs text-rose-600 dark:text-rose-400">
        {environmentStore.error}
      </p>
    {/if}
  </div>
  {/if}

  {#if activeSettingsTab === "engine"}
  {#if !isConnected}
    <div class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-amber-200 bg-amber-50/80 px-4 py-3 text-xs text-amber-800 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300">
      <span>
        {loading
          ? i18n.t.settings.connecting
          : i18n.t.settings.disconnected}
      </span>
      <button
        type="button"
        onclick={load}
        disabled={loading}
        class="rounded-md border border-amber-300 bg-white px-2.5 py-1 font-medium text-amber-800 transition hover:bg-amber-50 disabled:opacity-50 dark:border-amber-500/40 dark:bg-slate-900 dark:text-amber-300"
      >
        {i18n.t.settings.reconnect}
      </button>
    </div>
  {/if}

  <div class="space-y-6">
    <!-- Daemon & Automation -->
    <div class="space-y-4 rounded-2xl border border-slate-200 bg-white p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
      <div class="flex items-center justify-between border-b border-slate-200 pb-4 dark:border-slate-800">
        <div>
          <h3 class="text-sm font-semibold text-slate-900 dark:text-white">{i18n.t.settings.daemonSection}</h3>
          <p class="text-xs text-slate-400">{i18n.t.settings.daemonSectionDesc}</p>
        </div>
        <span class="rounded-md bg-slate-100 px-2.5 py-1 font-mono text-xs font-semibold text-slate-700 dark:bg-slate-800 dark:text-slate-200">
          {draft.distro_name}
        </span>
      </div>

      <div>
        <div class="flex items-center gap-1.5">
          <label
            for="idle-timeout"
            class="block text-sm font-medium text-slate-700 dark:text-slate-300"
          >
            {i18n.t.settings.idleLabel}
          </label>
          <HelpTip
            title={i18n.t.settings.idleLabel}
            align="left"
            text={i18n.t.settings.idleHelp}
          />
        </div>
        <p class="mt-0.5 text-xs text-slate-400">
          {i18n.t.settings.idleDesc}
        </p>
        <select
          id="idle-timeout"
          bind:value={draft.idle_timeout_secs}
          disabled={!isConnected}
          class="mt-2 w-full max-w-xs rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-sm text-slate-900 focus:border-sky-500 focus:outline-none disabled:opacity-60 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100"
        >
          {#each idleOptions as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
          {#if hasCustomIdle && draft.idle_timeout_secs !== null}
            <option value={draft.idle_timeout_secs}>
              {i18n.t.settings.idleCustom(draft.idle_timeout_secs)}
            </option>
          {/if}
        </select>
      </div>

      <div class="space-y-3 border-t border-slate-200 pt-4 dark:border-slate-800">
        <div class="flex items-center gap-2">
          <label class="flex items-center gap-3 text-sm text-slate-700 dark:text-slate-300">
            <input
              type="checkbox"
              bind:checked={draft.auto_trim_on_events}
              disabled={!isConnected}
              class="h-4 w-4 accent-sky-600 disabled:opacity-60"
            />
            {i18n.t.settings.autoTrimLabel}
          </label>
          <HelpTip text={i18n.t.settings.autoTrimHelp} />
        </div>

        <label class="flex items-center gap-3 text-sm text-slate-700 dark:text-slate-300">
          <input
            type="checkbox"
            bind:checked={draft.auto_start_on_login}
            disabled={!isConnected}
            class="h-4 w-4 accent-sky-600 disabled:opacity-60"
          />
          {i18n.t.settings.autoStartLabel}
        </label>
      </div>
    </div>

    <!-- WSL2 VM Resources & Networking -->
    <div class="space-y-5 rounded-2xl border border-slate-200 bg-white p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
      <div class="flex flex-wrap items-center justify-between gap-3 border-b border-slate-200 pb-4 dark:border-slate-800">
        <div>
          <h3 class="text-sm font-semibold text-slate-900 dark:text-white">{i18n.t.settings.wslSection}</h3>
          <p class="text-xs text-slate-400">{i18n.t.settings.wslSectionDesc}</p>
        </div>
        <div class="flex items-center gap-1.5">
          <span class="text-[11px] text-slate-400">{i18n.t.extra.profilesTitle}:</span>
          <button
            type="button"
            disabled={!isConnected}
            onclick={() => applyResourceProfile("eco")}
            class={`rounded-md border px-2 py-1 text-[11px] font-medium transition disabled:opacity-40 ${
              activeProfile === "eco"
                ? "border-sky-300 bg-sky-50 text-sky-700 dark:border-sky-500/40 dark:bg-sky-500/20 dark:text-sky-300"
                : "border-slate-200 text-slate-600 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
            }`}
          >
            {i18n.t.extra.profileEco}
          </button>
          <button
            type="button"
            disabled={!isConnected}
            onclick={() => applyResourceProfile("balanced")}
            class={`rounded-md border px-2 py-1 text-[11px] font-medium transition disabled:opacity-40 ${
              activeProfile === "balanced"
                ? "border-sky-300 bg-sky-50 text-sky-700 dark:border-sky-500/40 dark:bg-sky-500/20 dark:text-sky-300"
                : "border-slate-200 text-slate-600 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
            }`}
          >
            {i18n.t.extra.profileBalanced}
          </button>
          <button
            type="button"
            disabled={!isConnected}
            onclick={() => applyResourceProfile("perf")}
            class={`rounded-md border px-2 py-1 text-[11px] font-medium transition disabled:opacity-40 ${
              activeProfile === "perf"
                ? "border-sky-300 bg-sky-50 text-sky-700 dark:border-sky-500/40 dark:bg-sky-500/20 dark:text-sky-300"
                : "border-slate-200 text-slate-600 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
            }`}
          >
            {i18n.t.extra.profilePerf}
          </button>
        </div>
      </div>

      <div>
        <div class="flex flex-wrap items-center justify-between gap-2">
          <div class="flex items-center gap-1.5">
            <label
              for="ram-range"
              class="text-sm font-medium text-slate-700 dark:text-slate-300"
            >
              {i18n.t.settings.memoryLabel}
            </label>
            <HelpTip align="left" text={i18n.t.settings.memoryHelp} />
          </div>
          <span class="font-mono text-sm font-semibold text-sky-700 dark:text-sky-400">
            {draft.wsl.memory_mb} MB ({formatGb(draft.wsl.memory_mb)})
          </span>
        </div>
        <input
          id="ram-range"
          type="range"
          min="1024"
          max="32768"
          step="512"
          bind:value={draft.wsl.memory_mb}
          disabled={!isConnected}
          class="mt-2 w-full accent-sky-600 disabled:opacity-60"
        />
        <div class="mt-2 flex flex-wrap gap-1.5">
          {#each memoryPresets as preset}
            <button
              type="button"
              disabled={!isConnected}
              onclick={() => (draft.wsl.memory_mb = preset)}
              class={`rounded-md border px-2 py-0.5 text-[11px] font-medium transition disabled:opacity-50 ${
                draft.wsl.memory_mb === preset
                  ? "border-sky-300 bg-sky-50 text-sky-700 dark:border-sky-500/40 dark:bg-sky-500/20 dark:text-sky-300"
                  : "border-slate-200 text-slate-600 hover:bg-slate-50 dark:border-slate-700 dark:text-slate-400 dark:hover:bg-slate-800"
              }`}
            >
              {formatGb(preset)}
            </button>
          {/each}
        </div>
      </div>

      <div>
        <div class="flex justify-between text-sm font-medium text-slate-700 dark:text-slate-300">
          <div class="flex items-center gap-1.5">
            <label for="swap-range">{i18n.t.settings.swapLabel}</label>
            <HelpTip align="left" text={i18n.t.settings.swapHelp} />
          </div>
          <span class="font-mono font-semibold text-sky-700 dark:text-sky-400">
            {draft.wsl.swap_mb} MB ({formatGb(draft.wsl.swap_mb)})
          </span>
        </div>
        <input
          id="swap-range"
          type="range"
          min="0"
          max="16384"
          step="512"
          bind:value={draft.wsl.swap_mb}
          disabled={!isConnected}
          class="mt-2 w-full accent-sky-600 disabled:opacity-60"
        />
      </div>

      <div class="space-y-3 border-t border-slate-200 pt-4 dark:border-slate-800">
        <div class="flex items-center gap-2">
          <label class="flex items-center gap-3 text-sm text-slate-700 dark:text-slate-300">
            <input
              type="checkbox"
              bind:checked={draft.wsl.sparse_vhd}
              disabled={!isConnected}
              class="h-4 w-4 accent-sky-600 disabled:opacity-60"
            />
            {i18n.t.settings.sparseVhdLabel}
          </label>
          <HelpTip text={i18n.t.settings.sparseVhdHelp} />
        </div>

        <div class="flex items-center gap-2">
          <label class="flex items-center gap-3 text-sm text-slate-700 dark:text-slate-300">
            <input
              type="checkbox"
              bind:checked={draft.wsl.mirrored_networking}
              disabled={!isConnected}
              class="h-4 w-4 accent-sky-600 disabled:opacity-60"
            />
            {i18n.t.settings.mirroredNetLabel}
          </label>
          <HelpTip text={i18n.t.settings.mirroredNetHelp} />
        </div>
      </div>

      {#if wslDirty && engineStore.state === "running"}
        <div class="rounded-xl border border-amber-200/80 bg-amber-50/70 px-4 py-2.5 text-xs text-amber-800 dark:border-amber-500/25 dark:bg-amber-500/10 dark:text-amber-300">
          {i18n.t.settings.wslRestartNotice}
        </div>
      {/if}
    </div>

    <!-- Action Footer -->
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="flex flex-wrap items-center gap-3">
        <button
          type="button"
          onclick={() => save(false)}
          disabled={!isConnected || !isDirty || saving || engineStore.busy}
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-sm font-medium text-white shadow-xs transition hover:bg-sky-500 disabled:opacity-50"
        >
          {#if saving && !savingRestart}
            <Icon name="spinner" class="h-4 w-4 animate-spin" />
          {/if}
          {saving && !savingRestart ? i18n.t.settings.saving : i18n.t.settings.saveSettings}
        </button>
        {#if wslDirty && engineStore.state === "running"}
          <button
            type="button"
            onclick={() => save(true)}
            disabled={!isConnected || saving || engineStore.busy}
            class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-4 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
          >
            {#if saving && savingRestart}
              <Icon name="spinner" class="h-4 w-4 animate-spin" />
            {/if}
            {saving && savingRestart ? i18n.t.settings.saving : i18n.t.settings.saveAndRestart}
          </button>
        {/if}
        {#if isDirty}
          <button
            type="button"
            onclick={resetDraft}
            disabled={saving}
            class="rounded-lg px-3 py-2 text-sm font-medium text-slate-500 hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-slate-800"
          >
            {i18n.t.settings.reset}
          </button>
        {/if}
      </div>

      <button
        type="button"
        onclick={restoreDefaults}
        disabled={!isConnected || isDefault || saving}
        class="rounded-lg border border-slate-200 bg-white px-3.5 py-2 text-xs font-medium text-slate-600 transition hover:bg-slate-50 disabled:opacity-40 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300 dark:hover:bg-slate-700"
      >
        {i18n.t.settings.restoreDefaults}
      </button>
    </div>
  </div>
  {/if}

  {#if activeSettingsTab === "system"}
  <!-- WSL2 Distro Integration & Diagnostics -->
  <div class="space-y-4 rounded-2xl border border-slate-200 bg-white p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <h3 class="text-sm font-semibold text-slate-900 dark:text-white">{i18n.t.extra.wslDistrosTitle}</h3>
        <p class="text-xs text-slate-400">{i18n.t.extra.wslDistrosDesc}</p>
      </div>
      <button
        type="button"
        onclick={() => (diagnosticsOpen = true)}
        class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 text-xs font-medium text-slate-700 transition hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
      >
        <Icon name="activity" class="h-3.5 w-3.5 text-sky-500" />
        {i18n.t.extra.runDoctor}
      </button>
    </div>

    {#if wslDistros.length === 0}
      <p class="text-xs text-slate-400">{i18n.t.extra.wslDistrosEmpty}</p>
    {:else}
      <div class="divide-y divide-slate-200 rounded-xl border border-slate-200 dark:divide-slate-800 dark:border-slate-800">
        {#each wslDistros as distro}
          <div class="flex items-center justify-between px-4 py-2.5">
            <div>
              <span class="text-sm font-medium text-slate-800 dark:text-slate-200">{distro.name}</span>
              <span class="ml-2 rounded bg-slate-100 px-1.5 py-0.5 text-[10px] font-mono text-slate-500 dark:bg-slate-800 dark:text-slate-400">
                WSL{distro.version} · {distro.state}
              </span>
            </div>
            <input
              type="checkbox"
              checked={distro.integrated}
              disabled={distro.version !== 2 || distroBusy === distro.name}
              onchange={() => void toggleDistroIntegration(distro)}
              class="h-4 w-4 accent-sky-600 disabled:opacity-40"
            />
          </div>
        {/each}
      </div>
    {/if}
  </div>
  {/if}

  {#if isDirty}
    <div class="sticky bottom-4 z-30 flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-amber-200 bg-white/95 px-5 py-3 shadow-lg backdrop-blur-xs dark:border-amber-500/30 dark:bg-slate-900/95">
      <div class="flex items-center gap-2 text-xs font-medium text-amber-700 dark:text-amber-300">
        <span class="h-2 w-2 rounded-full bg-amber-500"></span>
        <span>{i18n.t.settings.unsavedChanges}</span>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onclick={resetDraft}
          disabled={saving}
          class="rounded-lg px-3 py-1.5 text-xs font-medium text-slate-600 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800"
        >
          {i18n.t.settings.reset}
        </button>
        {#if wslDirty && engineStore.state === "running"}
          <button
            type="button"
            onclick={() => save(true)}
            disabled={!isConnected || saving || engineStore.busy}
            class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200"
          >
            {saving && savingRestart ? i18n.t.settings.saving : i18n.t.settings.saveAndRestart}
          </button>
        {/if}
        <button
          type="button"
          onclick={() => save(false)}
          disabled={!isConnected || saving || engineStore.busy}
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-1.5 text-xs font-medium text-white shadow-xs transition hover:bg-sky-500 disabled:opacity-50"
        >
          {#if saving && !savingRestart}
            <Icon name="spinner" class="h-3.5 w-3.5 animate-spin" />
          {/if}
          {saving && !savingRestart ? i18n.t.settings.saving : i18n.t.settings.saveSettings}
        </button>
      </div>
    </div>
  {/if}
</section>

<DiagnosticsModal open={diagnosticsOpen} onClose={() => (diagnosticsOpen = false)} />
