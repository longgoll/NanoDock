<script lang="ts">
  import { onMount } from "svelte";
  import HelpTip from "../../lib/components/HelpTip.svelte";
  import Icon from "../../lib/components/Icon.svelte";
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
        <h2 class="text-lg font-semibold text-slate-900 dark:text-white">Virtual Disk (.vhdx) &amp; Storage</h2>
        <HelpTip
          title="How Disk Reclamation Works"
          align="left"
          position="bottom"
          text="Deleting containers or images frees blocks inside Alpine Linux, but Windows keeps the .vhdx size until fstrim reports those free blocks back to NTFS."
        />
      </div>
      <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
        Sparse VHD is <strong>{sparseEnabled ? "enabled" : "disabled"}</strong>. Run
        <code class="rounded bg-slate-100 px-1 py-0.5 font-mono text-xs dark:bg-slate-800">fstrim</code>
        to immediately return freed blocks to Windows NTFS.
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
        Refresh
      </button>
      <button
        type="button"
        onclick={() => engineStore.trim()}
        disabled={engineStore.busy}
        class="rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white shadow-xs transition hover:bg-emerald-500 disabled:opacity-50"
      >
        {engineStore.busy ? "Trimming..." : "Reclaim Disk (fstrim)"}
      </button>
    </div>
  </div>

  <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
    <div class="rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40">
      <div class="flex items-center gap-1.5">
        <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
          Current VHDX Size
        </p>
        <HelpTip
          align="left"
          text="Actual disk space occupied by NanoDock's ext4.vhdx virtual disk on your Windows NTFS drive."
        />
      </div>
      <p class="mt-2 text-3xl font-bold text-slate-900 dark:text-white">
        {formatBytes(engineStore.vhdxSizeBytes)}
      </p>
      <p class="mt-1 text-xs text-slate-400">
        Actual footprint on Windows NTFS
      </p>
    </div>

    <div class="rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40">
      <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
        Reclaim Tip
      </p>
      <p class="mt-2 text-sm leading-relaxed text-slate-600 dark:text-slate-300">
        Delete unused images and volumes first, then click <strong>Reclaim Disk (fstrim)</strong> to shrink the virtual disk file.
      </p>
    </div>
  </div>
</section>
