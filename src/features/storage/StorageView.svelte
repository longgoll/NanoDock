<script lang="ts">
  import { engineStore } from "../../lib/stores/engine.svelte";
</script>

<section class="space-y-6">
  <div class="flex items-center justify-between rounded-2xl border border-slate-200 bg-white p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
    <div>
      <h2 class="text-lg font-semibold text-slate-900 dark:text-white">Virtual Disk (.vhdx) & Storage</h2>
      <p class="mt-1 text-sm text-slate-500 dark:text-slate-400">
        Sparse VHD enabled. Run fstrim to immediately return freed blocks to Windows NTFS.
      </p>
    </div>
    <button
      onclick={() => engineStore.trim()}
      disabled={engineStore.busy}
      class="rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white shadow-xs transition hover:bg-emerald-500 disabled:opacity-50"
    >
      Reclaim Disk (fstrim)
    </button>
  </div>
</section>
