<script lang="ts">
  import { onMount } from "svelte";
  import HelpTip from "../../lib/components/HelpTip.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import { i18n } from "../../lib/i18n/index.svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";
  import { formatBytes } from "../../lib/utils/format";

  let refreshing = $state(false);

  onMount(() => {
    if (!engineStore.config) {
      void engineStore.loadConfig();
    }
  });

  async function refreshSize() {
    refreshing = true;
    try {
      await engineStore.refresh();
    } finally {
      refreshing = false;
    }
  }

  const sparseEnabled = $derived(engineStore.config?.wsl.sparse_vhd ?? true);
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
      <button
        type="button"
        onclick={refreshSize}
        disabled={refreshing || engineStore.busy}
        class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-3.5 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
      >
        <Icon name="refresh" class={`h-4 w-4 ${refreshing ? "animate-spin" : ""}`} />
        {i18n.t.common.refresh}
      </button>
      <button
        type="button"
        onclick={() => engineStore.trim()}
        disabled={engineStore.busy}
        class="rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white shadow-xs transition hover:bg-emerald-500 disabled:opacity-50"
      >
        {engineStore.busy ? i18n.t.storage.trimming : i18n.t.storage.reclaimButton}
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
</section>
