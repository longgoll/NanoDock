<script lang="ts">
  import { onMount } from "svelte";
  import { daemonApi } from "../../lib/api/daemon";
  import { engineStore } from "../../lib/stores/engine.svelte";

  let query = $state("");
  let logsModalOpen = $state(false);
  let logsTitle = $state("");
  let logsContent = $state("");
  let loadingLogs = $state(false);

  onMount(() => {
    engineStore.refreshContainers();
  });

  const filtered = $derived(
    engineStore.containers.filter((c) => {
      const q = query.trim().toLowerCase();
      if (!q) return true;
      return (
        c.names.some((n) => n.toLowerCase().includes(q)) ||
        c.image.toLowerCase().includes(q) ||
        c.id.toLowerCase().includes(q) ||
        (c.compose_project?.toLowerCase().includes(q) ?? false)
      );
    }),
  );

  async function viewLogs(id: string, name: string) {
    logsTitle = name || id;
    logsContent = "";
    logsModalOpen = true;
    loadingLogs = true;
    try {
      const resp = await daemonApi.getContainerLogs(id, 200);
      if (resp.status === "container_logs") {
        logsContent = resp.data.logs || "(No logs output)";
      } else if (resp.status === "error") {
        logsContent = `Error: ${resp.data.message}`;
      }
    } catch (e) {
      logsContent = `Error: ${String(e)}`;
    } finally {
      loadingLogs = false;
    }
  }

  function stateBadgeClass(state: string): string {
    if (state === "running") {
      return "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-500/30 dark:bg-emerald-500/15 dark:text-emerald-400";
    }
    if (state === "paused") {
      return "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/15 dark:text-amber-400";
    }
    return "border-slate-200 bg-slate-100 text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-400";
  }
</script>

<section class="space-y-5">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <h2 class="text-lg font-semibold text-slate-900 dark:text-white">Containers</h2>
      <p class="text-xs text-slate-500 dark:text-slate-400">
        Manage running and stopped containers in NanoDock
      </p>
    </div>

    <div class="flex items-center gap-2.5">
      <input
        type="text"
        bind:value={query}
        placeholder="Filter by name, image, stack..."
        class="w-60 rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-sm text-slate-900 placeholder-slate-400 focus:border-sky-500 focus:outline-none dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100"
      />
      <button
        onclick={() => engineStore.refreshContainers()}
        disabled={engineStore.loadingResources}
        class="rounded-lg border border-slate-200 bg-white px-3.5 py-1.5 text-sm font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
      >
        {engineStore.loadingResources ? "Loading..." : "Refresh"}
      </button>
    </div>
  </div>

  {#if filtered.length === 0}
    <div class="rounded-2xl border border-dashed border-slate-300 bg-white p-10 text-center text-sm text-slate-500 dark:border-slate-800 dark:bg-slate-900/30 dark:text-slate-400">
      No containers found. Run <code class="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-sky-700 dark:bg-slate-800 dark:text-sky-400">nanodock run -d -p 8080:80 nginx:alpine</code> to get started.
    </div>
  {:else}
    <div class="overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900/50">
      <table class="w-full text-left text-sm">
        <thead class="border-b border-slate-200 bg-slate-50 text-xs font-semibold uppercase tracking-wider text-slate-500 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-400">
          <tr>
            <th class="px-4 py-3">Name</th>
            <th class="px-4 py-3">Image</th>
            <th class="px-4 py-3">Status</th>
            <th class="px-4 py-3">Ports</th>
            <th class="px-4 py-3 text-right">Actions</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-slate-200 dark:divide-slate-800">
          {#each filtered as c (c.id)}
            {@const displayName = c.names[0] || c.id}
            <tr class="transition hover:bg-slate-50/80 dark:hover:bg-slate-800/40">
              <td class="px-4 py-3">
                <div class="font-medium text-slate-900 dark:text-white">{displayName}</div>
                <div class="flex items-center gap-2 text-xs text-slate-400">
                  <span class="font-mono">{c.id}</span>
                  {#if c.compose_project}
                    <span class="rounded bg-sky-50 px-1.5 py-0.5 text-[10px] font-medium text-sky-700 dark:bg-sky-500/15 dark:text-sky-400">
                      {c.compose_project}
                    </span>
                  {/if}
                </div>
              </td>
              <td class="px-4 py-3 font-mono text-xs text-slate-700 dark:text-slate-300">{c.image}</td>
              <td class="px-4 py-3">
                <span class={`inline-flex rounded-full border px-2.5 py-0.5 text-xs font-medium ${stateBadgeClass(c.state)}`}>
                  {c.state}
                </span>
                <div class="mt-0.5 text-xs text-slate-400">{c.status}</div>
              </td>
              <td class="px-4 py-3 font-mono text-xs text-slate-600 dark:text-slate-300">
                {c.ports.length > 0 ? c.ports.join(", ") : "—"}
              </td>
              <td class="px-4 py-3 text-right">
                <div class="inline-flex items-center gap-1.5">
                  {#if c.state === "running"}
                    <button
                      onclick={() => engineStore.runContainerAction(c.id, "stop")}
                      disabled={engineStore.busy}
                      class="rounded-md border border-slate-200 px-2.5 py-1 text-xs font-medium text-slate-700 hover:bg-slate-100 disabled:opacity-50 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
                    >
                      Stop
                    </button>
                    <button
                      onclick={() => engineStore.runContainerAction(c.id, "restart")}
                      disabled={engineStore.busy}
                      class="rounded-md border border-slate-200 px-2.5 py-1 text-xs font-medium text-slate-700 hover:bg-slate-100 disabled:opacity-50 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
                    >
                      Restart
                    </button>
                  {:else}
                    <button
                      onclick={() => engineStore.runContainerAction(c.id, "start")}
                      disabled={engineStore.busy}
                      class="rounded-md bg-sky-600 px-2.5 py-1 text-xs font-medium text-white hover:bg-sky-500 disabled:opacity-50"
                    >
                      Start
                    </button>
                  {/if}
                  <button
                    onclick={() => viewLogs(c.id, displayName)}
                    class="rounded-md border border-slate-200 px-2.5 py-1 text-xs font-medium text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
                  >
                    Logs
                  </button>
                  <button
                    onclick={() => engineStore.runContainerAction(c.id, "remove")}
                    disabled={engineStore.busy}
                    class="rounded-md border border-rose-200 px-2.5 py-1 text-xs font-medium text-rose-600 hover:bg-rose-50 disabled:opacity-50 dark:border-rose-500/30 dark:text-rose-400 dark:hover:bg-rose-500/15"
                  >
                    Delete
                  </button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

{#if logsModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/50 p-6 backdrop-blur-xs">
    <div class="flex max-h-[80vh] w-full max-w-3xl flex-col rounded-2xl border border-slate-200 bg-white shadow-xl dark:border-slate-800 dark:bg-slate-900">
      <div class="flex items-center justify-between border-b border-slate-200 px-5 py-3.5 dark:border-slate-800">
        <h3 class="text-sm font-semibold text-slate-900 dark:text-white">Logs: {logsTitle}</h3>
        <button
          onclick={() => (logsModalOpen = false)}
          class="rounded-lg px-2.5 py-1 text-xs font-medium text-slate-500 hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-slate-800"
        >
          Close
        </button>
      </div>
      <pre class="flex-1 overflow-auto bg-slate-50 p-4 font-mono text-xs leading-relaxed text-slate-800 dark:bg-slate-950 dark:text-slate-200">{loadingLogs ? "Loading logs..." : logsContent}</pre>
    </div>
  </div>
{/if}
