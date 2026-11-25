<script lang="ts">
  import { onMount } from "svelte";
  import { daemonApi, type EngineConfig } from "../../lib/api/daemon";
  import HelpTip from "../../lib/components/HelpTip.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import { i18n, type LocalePreference } from "../../lib/i18n/index.svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";

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

  let saving = $state(false);
  let loading = $state(false);
  let draft = $state<EngineConfig>(cloneConfig(DEFAULT_CONFIG));

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
    if (engineStore.config) {
      draft = cloneConfig(engineStore.config);
    }
    void load();
  });

  const isConnected = $derived(engineStore.config !== null);

  const isDirty = $derived.by(() => {
    if (!engineStore.config) return false;
    return JSON.stringify(draft) !== JSON.stringify(engineStore.config);
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
    try {
      const resp = await daemonApi.updateConfig(draft);
      if (resp.status === "ack") {
        const wslChanged = wslDirty;
        engineStore.config = cloneConfig(draft);
        if (restartAfter && engineStore.state !== "stopped") {
          await engineStore.stop();
          await engineStore.start();
          engineStore.setNotice("Settings saved and WSL2 engine restarted.");
        } else if (wslChanged && engineStore.state === "running") {
          engineStore.setNotice("Settings saved. Restart the engine to apply WSL2 memory/network changes.");
        } else {
          engineStore.setNotice("Settings saved.");
        }
      } else if (resp.status === "error") {
        engineStore.error = resp.data.message;
      }
    } catch (e) {
      engineStore.error = String(e);
    } finally {
      saving = false;
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
      <button
        type="button"
        onclick={load}
        disabled={loading || saving}
        class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
      >
        <Icon name="refresh" class={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
        {i18n.t.common.refresh}
      </button>
    </div>
  </div>

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

  {#if !isConnected}
    <div class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-amber-200 bg-amber-50/80 px-4 py-3 text-xs text-amber-800 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300">
      <span>
        {loading
          ? "Connecting to core-daemon..."
          : "Core daemon is not connected. Showing default configuration (read-only until connected)."}
      </span>
      <button
        type="button"
        onclick={load}
        disabled={loading}
        class="rounded-md border border-amber-300 bg-white px-2.5 py-1 font-medium text-amber-800 transition hover:bg-amber-50 disabled:opacity-50 dark:border-amber-500/40 dark:bg-slate-900 dark:text-amber-300"
      >
        Reconnect
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
      <div class="border-b border-slate-200 pb-4 dark:border-slate-800">
        <h3 class="text-sm font-semibold text-slate-900 dark:text-white">{i18n.t.settings.wslSection}</h3>
        <p class="text-xs text-slate-400">{i18n.t.settings.wslSectionDesc}</p>
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
          class="rounded-lg bg-sky-600 px-4 py-2 text-sm font-medium text-white shadow-xs transition hover:bg-sky-500 disabled:opacity-50"
        >
          {saving ? i18n.t.settings.saving : i18n.t.settings.saveSettings}
        </button>
        {#if wslDirty && engineStore.state === "running"}
          <button
            type="button"
            onclick={() => save(true)}
            disabled={!isConnected || saving || engineStore.busy}
            class="rounded-lg border border-slate-200 bg-white px-4 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
          >
            {i18n.t.settings.saveAndRestart}
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
</section>
