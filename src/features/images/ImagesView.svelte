<script lang="ts">
  import { onMount } from "svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";

  let query = $state("");
  let pullRef = $state("");

  onMount(() => {
    engineStore.refreshImages();
  });

  const filtered = $derived(
    engineStore.images.filter((img) => {
      const q = query.trim().toLowerCase();
      if (!q) return true;
      return (
        img.id.toLowerCase().includes(q) ||
        img.repo_tags.some((t) => t.toLowerCase().includes(q))
      );
    }),
  );

  function formatBytes(bytes: number): string {
    if (bytes >= 1024 * 1024 * 1024) {
      return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
    }
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  async function handlePull(e: SubmitEvent) {
    e.preventDefault();
    const ref = pullRef.trim();
    if (!ref) return;
    await engineStore.pullImage(ref);
    pullRef = "";
  }
</script>

<section class="space-y-5">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <h2 class="text-lg font-semibold text-slate-900 dark:text-white">Images</h2>
      <p class="text-xs text-slate-500 dark:text-slate-400">
        Pull, inspect, and clean up local container images
      </p>
    </div>

    <div class="flex flex-wrap items-center gap-2.5">
      <input
        type="text"
        bind:value={query}
        placeholder="Filter images..."
        class="w-48 rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-sm text-slate-900 placeholder-slate-400 focus:border-sky-500 focus:outline-none dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100"
      />
      <button
        onclick={() => engineStore.pruneImages()}
        disabled={engineStore.busy}
        class="rounded-lg border border-amber-200 bg-amber-50 px-3 py-1.5 text-sm font-medium text-amber-700 transition hover:bg-amber-100 disabled:opacity-50 dark:border-amber-500/30 dark:bg-amber-500/15 dark:text-amber-400"
      >
        Prune Unused
      </button>
      <button
        onclick={() => engineStore.refreshImages()}
        disabled={engineStore.loadingResources}
        class="rounded-lg border border-slate-200 bg-white px-3.5 py-1.5 text-sm font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
      >
        {engineStore.loadingResources ? "Loading..." : "Refresh"}
      </button>
    </div>
  </div>

  <form
    onsubmit={handlePull}
    class="flex items-center gap-2.5 rounded-2xl border border-slate-200 bg-white p-3.5 shadow-xs dark:border-slate-800 dark:bg-slate-900/50"
  >
    <input
      type="text"
      bind:value={pullRef}
      placeholder="Pull image (e.g. nginx:alpine, postgres:16-alpine)"
      class="flex-1 rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 text-sm text-slate-900 placeholder-slate-400 focus:border-sky-500 focus:bg-white focus:outline-none dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100"
    />
    <button
      type="submit"
      disabled={engineStore.busy || !pullRef.trim()}
      class="rounded-lg bg-sky-600 px-4 py-1.5 text-sm font-medium text-white transition hover:bg-sky-500 disabled:opacity-50"
    >
      {engineStore.busy ? "Working..." : "Pull Image"}
    </button>
  </form>

  {#if filtered.length === 0}
    <div class="rounded-2xl border border-dashed border-slate-300 bg-white p-10 text-center text-sm text-slate-500 dark:border-slate-800 dark:bg-slate-900/30 dark:text-slate-400">
      No images found. Pull an image above or run <code class="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-sky-700 dark:bg-slate-800 dark:text-sky-400">nanodock pull alpine</code>.
    </div>
  {:else}
    <div class="overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900/50">
      <table class="w-full text-left text-sm">
        <thead class="border-b border-slate-200 bg-slate-50 text-xs font-semibold uppercase tracking-wider text-slate-500 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-400">
          <tr>
            <th class="px-4 py-3">Repository : Tag</th>
            <th class="px-4 py-3">Image ID</th>
            <th class="px-4 py-3">Size</th>
            <th class="px-4 py-3 text-right">Actions</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-slate-200 dark:divide-slate-800">
          {#each filtered as img (img.id)}
            <tr class="transition hover:bg-slate-50/80 dark:hover:bg-slate-800/40">
              <td class="px-4 py-3">
                <div class="flex flex-wrap gap-1.5">
                  {#if img.repo_tags.length > 0}
                    {#each img.repo_tags as tag}
                      <span class="rounded-md bg-slate-100 px-2 py-0.5 font-mono text-xs font-medium text-slate-800 dark:bg-slate-800 dark:text-slate-200">
                        {tag}
                      </span>
                    {/each}
                  {:else}
                    <span class="font-mono text-xs text-slate-400">&lt;none&gt;</span>
                  {/if}
                </div>
              </td>
              <td class="px-4 py-3 font-mono text-xs text-slate-500 dark:text-slate-400">{img.id}</td>
              <td class="px-4 py-3 text-xs font-medium text-slate-700 dark:text-slate-300">
                {formatBytes(img.size)}
              </td>
              <td class="px-4 py-3 text-right">
                <button
                  onclick={() => engineStore.removeImage(img.id)}
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
