<script lang="ts">
  import { onMount } from "svelte";
  import HelpTip from "../../lib/components/HelpTip.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import RefreshButton from "../../lib/components/RefreshButton.svelte";
  import { i18n } from "../../lib/i18n/index.svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";
  import { formatBytes } from "../../lib/utils/format";

  onMount(() => {
    if (!engineStore.config) {
      void engineStore.loadConfig();
    }
    if (engineStore.state === "running") {
      void engineStore.refreshBuildCache();
    }
  });

  const sparseEnabled = $derived(engineStore.config?.wsl.sparse_vhd ?? true);
  const isRunning = $derived(engineStore.state === "running");
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
</section>
