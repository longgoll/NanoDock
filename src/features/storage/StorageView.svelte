<script lang="ts">
  import { onMount } from "svelte";
  import {
    daemonApi,
    type LocalProxyRoute,
    type TurboMountStatus,
  } from "../../lib/api/daemon";
  import HelpTip from "../../lib/components/HelpTip.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import RefreshButton from "../../lib/components/RefreshButton.svelte";
  import { i18n } from "../../lib/i18n/index.svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";
  import { formatBytes } from "../../lib/utils/format";

  let turboMounts = $state<TurboMountStatus[]>([]);
  let proxyRoutes = $state<LocalProxyRoute[]>([]);
  let newTurboPath = $state("");
  let turboBusy = $state(false);

  async function refreshTurboAndRoutes() {
    const [turboRes, routesRes] = await Promise.all([
      daemonApi.listTurboMounts(),
      daemonApi.listLocalProxyRoutes(),
    ]);
    if (turboRes.status === "turbo_mounts") {
      turboMounts = turboRes.data;
    }
    if (routesRes.status === "local_proxy_routes") {
      proxyRoutes = routesRes.data;
    }
  }

  async function addTurboMount() {
    const path = newTurboPath.trim();
    if (!path) return;
    turboBusy = true;
    try {
      const res = await daemonApi.addTurboMount(path);
      if (res.status === "turbo_mounts") {
        turboMounts = res.data;
        newTurboPath = "";
      } else if (res.status === "error") {
        engineStore.error = res.data.message;
      }
    } finally {
      turboBusy = false;
    }
  }

  async function toggleTurboMount(item: TurboMountStatus) {
    const res = await daemonApi.toggleTurboMount(item.id, !item.enabled);
    if (res.status === "turbo_mounts") {
      turboMounts = res.data;
    }
  }

  async function removeTurboMount(id: string) {
    const res = await daemonApi.removeTurboMount(id);
    if (res.status === "turbo_mounts") {
      turboMounts = res.data;
    }
  }

  async function syncTurboMount(id?: string) {
    turboBusy = true;
    try {
      const res = await daemonApi.triggerTurboSync(id);
      if (res.status === "turbo_mounts") {
        turboMounts = res.data;
      }
    } finally {
      turboBusy = false;
    }
  }

  onMount(() => {
    if (!engineStore.config) {
      void engineStore.loadConfig();
    }
    if (engineStore.state === "running") {
      void engineStore.refreshBuildCache();
    }
    void refreshTurboAndRoutes();
  });

  const sparseEnabled = $derived(engineStore.config?.wsl.sparse_vhd ?? true);
  const isRunning = $derived(engineStore.state === "running");
  const proxyPort = $derived(engineStore.config?.local_proxy_port ?? 8090);
</script>

<section class="space-y-6">
  <div class="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-slate-200 bg-white p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
    <div>
      <div class="flex items-center gap-2">
        <h2 class="text-lg font-semibold text-slate-900 dark:text-white">
          {i18n.t.storage.title}
        </h2>
        <HelpTip
          title={i18n.t.storage.helpTitle}
          align="left"
          position="bottom"
          text={i18n.t.storage.helpText}
        />
      </div>
      <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
        {i18n.t.storage.subtitlePrefix}
        <strong>{sparseEnabled ? i18n.t.dashboard.enabled.toLowerCase() : i18n.t.dashboard.disabled.toLowerCase()}</strong>{i18n.t.storage.subtitleMiddle}
        <code class="rounded bg-slate-100 px-1 py-0.5 font-mono text-xs dark:bg-slate-800">fstrim</code>
        {i18n.t.storage.subtitleSuffix}
      </p>
    </div>
    <div class="flex items-center gap-2.5">
      <RefreshButton
        size="md"
        onRefresh={async () => {
          await engineStore.refresh();
          if (isRunning) await engineStore.refreshBuildCache();
        }}
        disabled={engineStore.busy}
      />
      <button
        type="button"
        onclick={() => engineStore.trim()}
        disabled={engineStore.busy}
        class="inline-flex items-center gap-1.5 rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white shadow-xs transition hover:bg-emerald-500 disabled:opacity-50"
      >
        {#if engineStore.isPending("engine:trim")}
          <Icon name="spinner" class="h-4 w-4 animate-spin" />
        {/if}
        {engineStore.isPending("engine:trim") ? i18n.t.storage.trimming : i18n.t.storage.reclaimButton}
      </button>
    </div>
  </div>

  <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
    <div class="rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40">
      <div class="flex items-center gap-1.5">
        <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
          {i18n.t.storage.currentSize}
        </p>
        <HelpTip align="left" text={i18n.t.storage.actualFootprint} />
      </div>
      <p class="mt-2 text-3xl font-bold text-slate-900 dark:text-white">
        {formatBytes(engineStore.vhdxSizeBytes)}
      </p>
      <p class="mt-1 text-xs text-slate-400">
        {i18n.t.storage.actualFootprint}
      </p>
    </div>

    <div class="rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40">
      <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
        {i18n.t.storage.reclaimTipTitle}
      </p>
      <p class="mt-2 text-sm leading-relaxed text-slate-600 dark:text-slate-300">
        {i18n.t.storage.reclaimTipBody}
      </p>
    </div>
  </div>

  <div class="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40">
    <div>
      <div class="flex items-center gap-1.5">
        <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
          {i18n.t.storage.buildCacheTitle}
        </p>
        <HelpTip align="left" text={i18n.t.storage.buildCacheHint} />
      </div>
      {#if isRunning}
        <p class="mt-2 text-2xl font-bold text-slate-900 dark:text-white">
          {formatBytes(engineStore.buildCache?.total_bytes ?? null)}
        </p>
        <p class="mt-1 text-xs text-slate-500 dark:text-slate-400">
          {#if engineStore.buildCache}
            {i18n.t.storage.buildCacheReclaimable(
              formatBytes(engineStore.buildCache.reclaimable_bytes),
              engineStore.buildCache.record_count,
            )}
          {:else}
            {i18n.t.storage.buildCacheHint}
          {/if}
        </p>
      {:else}
        <p class="mt-2 text-sm text-slate-500 dark:text-slate-400">
          {i18n.t.storage.buildCacheSleeping}
        </p>
      {/if}
    </div>
    <button
      type="button"
      onclick={() => engineStore.pruneBuildCache()}
      disabled={!isRunning || engineStore.busy || engineStore.isPending("storage:prune_build_cache")}
      class="inline-flex items-center gap-1.5 rounded-lg border border-slate-300 bg-white px-4 py-2 text-sm font-medium text-slate-700 shadow-xs transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
    >
      {#if engineStore.isPending("storage:prune_build_cache")}
        <Icon name="spinner" class="h-4 w-4 animate-spin" />
      {/if}
      {engineStore.isPending("storage:prune_build_cache")
        ? i18n.t.storage.pruningBuildCache
        : i18n.t.storage.pruneBuildCache}
    </button>
  </div>

  <!-- Turbo Mounts (Windows -> WSL2 ext4 bind-mount acceleration) -->
  <div class="space-y-4 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40">
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div>
        <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
          {i18n.t.storage.turboTitle}
        </p>
        <p class="mt-1 text-xs text-slate-500 dark:text-slate-400">
          {i18n.t.storage.turboDesc}
        </p>
      </div>
      <button
        type="button"
        onclick={() => syncTurboMount()}
        disabled={!isRunning || turboBusy || turboMounts.length === 0}
        class="rounded-lg border border-slate-300 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200"
      >
        {i18n.t.storage.turboSyncAll}
      </button>
    </div>

    <div class="flex gap-2">
      <input
        type="text"
        bind:value={newTurboPath}
        placeholder={i18n.t.storage.turboPlaceholder}
        class="flex-1 rounded-lg border border-slate-300 bg-white px-3 py-1.5 text-sm text-slate-900 focus:border-sky-500 focus:outline-none dark:border-slate-700 dark:bg-slate-800 dark:text-slate-100"
      />
      <button
        type="button"
        onclick={addTurboMount}
        disabled={turboBusy || !newTurboPath.trim()}
        class="rounded-lg bg-sky-600 px-3.5 py-1.5 text-xs font-medium text-white transition hover:bg-sky-500 disabled:opacity-50"
      >
        {i18n.t.storage.turboAddBtn}
      </button>
    </div>

    {#if turboMounts.length > 0}
      <div class="divide-y divide-slate-200 rounded-xl border border-slate-200 dark:divide-slate-800 dark:border-slate-800">
        {#each turboMounts as item (item.id)}
          <div class="flex flex-wrap items-center justify-between gap-3 px-4 py-2.5 text-xs">
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <span class="font-mono font-semibold text-slate-900 dark:text-white">{item.windows_path}</span>
                <span class="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-[10px] text-slate-600 dark:bg-slate-800 dark:text-slate-300">
                  {item.state} ({i18n.t.storage.turboFilesCount(item.files_synced)})
                </span>
              </div>
              <p class="mt-0.5 truncate font-mono text-[11px] text-slate-400">
                → {item.wsl_path}
              </p>
              {#if item.last_error}
                <p class="mt-0.5 text-[11px] text-rose-500">{item.last_error}</p>
              {/if}
            </div>
            <div class="flex items-center gap-2">
              <button
                type="button"
                onclick={() => toggleTurboMount(item)}
                class="rounded border border-slate-300 px-2 py-1 text-[11px] dark:border-slate-700"
              >
                {item.enabled ? i18n.t.storage.turboEnabled : i18n.t.storage.turboPaused}
              </button>
              <button
                type="button"
                onclick={() => syncTurboMount(item.id)}
                disabled={!isRunning || turboBusy}
                class="rounded border border-slate-300 px-2 py-1 text-[11px] disabled:opacity-50 dark:border-slate-700"
              >
                {i18n.t.storage.turboSyncBtn}
              </button>
              <button
                type="button"
                onclick={() => removeTurboMount(item.id)}
                class="rounded border border-rose-300 px-2 py-1 text-[11px] text-rose-600 dark:border-rose-800 dark:text-rose-400"
              >
                {i18n.t.storage.turboRemoveBtn}
              </button>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Local *.localhost Reverse Proxy Routes -->
  <div class="space-y-3 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40">
    <div class="flex items-center justify-between">
      <div>
        <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
          {i18n.t.storage.proxyTitle(proxyPort)}
        </p>
        <p class="mt-1 text-xs text-slate-500 dark:text-slate-400">
          {i18n.t.storage.proxyDesc(proxyPort)}
        </p>
      </div>
      <button
        type="button"
        onclick={refreshTurboAndRoutes}
        class="rounded-lg border border-slate-300 px-3 py-1.5 text-xs font-medium text-slate-700 dark:border-slate-700 dark:text-slate-200"
      >
        {i18n.t.storage.proxyRefresh}
      </button>
    </div>
    {#if proxyRoutes.length === 0}
      <p class="text-xs text-slate-400">{i18n.t.storage.proxyEmpty}</p>
    {:else}
      <div class="flex flex-wrap gap-2">
        {#each proxyRoutes as route (route.host)}
          <span class="rounded-lg border border-slate-200 bg-slate-50 px-2.5 py-1 font-mono text-xs text-slate-700 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200">
            http://{route.host}:{proxyPort} → :{route.target_port}
          </span>
        {/each}
      </div>
    {/if}
  </div>
</section>
