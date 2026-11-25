<script lang="ts">
  import { onMount } from "svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";

  let query = $state("");

  onMount(() => {
    engineStore.refreshVolumes();
  });

  const filtered = $derived(
    engineStore.volumes.filter((v) => {
      const q = query.trim().toLowerCase();
      if (!q) return true;
      return (
        v.name.toLowerCase().includes(q) ||
        v.driver.toLowerCase().includes(q) ||
        v.mountpoint.toLowerCase().includes(q)
      );
    }),
  );
</script>

<section class="space-y-5">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <h2 class="text-lg font-semibold text-slate-900 dark:text-white">Volumes</h2>
      <p class="text-xs text-slate-500 dark:text-slate-400">
        Persistent storage volumes inside the NanoDock WSL2 filesystem
      </p>
    </div>

    <div class="flex items-center gap-2.5">
      <input
        type="text"
        bind:value={query}
        placeholder="Filter volumes..."
        class="w-48 rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-sm text-slate-900 placeholder-slate-400 focus:border-sky-500 focus:outline-none dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100"
      />
      <button
        onclick={() => engineStore.pruneVolumes()}
        disabled={engineStore.busy}
        class="rounded-lg border border-amber-200 bg-amber-50 px-3 py-1.5 text-sm font-medium text-amber-700 transition hover:bg-amber-100 disabled:opacity-50 dark:border-amber-500/30 dark:bg-amber-500/15 dark:text-amber-400"
      >
        Prune Unused
      </button>
      <button
        onclick={() => engineStore.refreshVolumes()}
        disabled={engineStore.loadingResources}
        class="rounded-lg border border-slate-200 bg-white px-3.5 py-1.5 text-sm font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
      >
        {engineStore.loadingResources ? "Loading..." : "Refresh"}
      </button>
    </div>
  </div>

  {#if filtered.length === 0}
    <div class="rounded-2xl border border-dashed border-slate-300 bg-white p-10 text-center text-sm text-slate-500 dark:border-slate-800 dark:bg-slate-900/30 dark:text-slate-400">
      No volumes found. Create one with <code class="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-sky-700 dark:bg-slate-800 dark:text-sky-400">nanodock volume create mydata</code>.
    </div>
  {:else}
    <div class="overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900/50">
      <table class="w-full text-left text-sm">
        <thead class="border-b border-slate-200 bg-slate-50 text-xs font-semibold uppercase tracking-wider text-slate-500 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-400">
          <tr>
            <th class="px-4 py-3">Name</th>
            <th class="px-4 py-3">Driver</th>
            <th class="px-4 py-3">Mountpoint</th>
            <th class="px-4 py-3 text-right">Actions</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-slate-200 dark:divide-slate-800">
          {#each filtered as vol (vol.name)}
            <tr class="transition hover:bg-slate-50/80 dark:hover:bg-slate-800/40">
              <td class="px-4 py-3 font-medium text-slate-900 dark:text-white">{vol.name}</td>
              <td class="px-4 py-3">
                <span class="rounded-md bg-slate-100 px-2 py-0.5 text-xs font-medium text-slate-700 dark:bg-slate-800 dark:text-slate-300">
                  {vol.driver}
                </span>
              </td>
              <td class="max-w-xs truncate px-4 py-3 font-mono text-xs text-slate-500 dark:text-slate-400" title={vol.mountpoint}>
                {vol.mountpoint}
              </td>
              <td class="px-4 py-3 text-right">
                <button
                  onclick={() => engineStore.removeVolume(vol.name)}
                  disabled={engineStore.busy}
                  class="rounded-md border border-rose-200 px-2.5 py-1 text-xs font-medium text-rose-600 hover:bg-rose-50 disabled:opacity-50 dark:border-rose-500/30 dark:text-rose-400 dark:hover:bg-rose-500/15"
                >
                  Delete
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>
