<script lang="ts">
  import { onMount } from "svelte";
  import type { NetworkSummary } from "../../lib/api/daemon";
  import ConfirmModal from "../../lib/components/ConfirmModal.svelte";
  import EngineSleepingCard from "../../lib/components/EngineSleepingCard.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import InUseBadge from "../../lib/components/InUseBadge.svelte";
  import RefreshButton from "../../lib/components/RefreshButton.svelte";
  import SearchInput from "../../lib/components/SearchInput.svelte";
  import { i18n } from "../../lib/i18n/index.svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";
  import { useConfirm } from "../../lib/utils/confirm.svelte";

  type UsageFilter = "all" | "in_use" | "unused";
  const BUILTIN_NETWORKS = new Set(["bridge", "host", "none"]);

  let query = $state("");
  let usageFilter = $state<UsageFilter>("all");
  let createModalOpen = $state(false);
  let newName = $state("");
  let newDriver = $state("bridge");
  let newInternal = $state(false);

  const confirmState = useConfirm();

  onMount(() => {
    void engineStore.refreshNetworks(engineStore.networks.length > 0);
  });

  const totalCount = $derived(engineStore.networks.length);
  const inUseCount = $derived(engineStore.networks.filter((n) => n.in_use).length);
  const unusedCount = $derived(totalCount - inUseCount);

  const filtered = $derived(
    engineStore.networks.filter((n) => {
      if (usageFilter === "in_use" && !n.in_use) return false;
      if (usageFilter === "unused" && n.in_use) return false;
      const q = query.trim().toLowerCase();
      if (!q) return true;
      return (
        n.name.toLowerCase().includes(q) ||
        n.driver.toLowerCase().includes(q) ||
        n.id.toLowerCase().includes(q) ||
        n.used_by.some((c) => c.toLowerCase().includes(q))
      );
    }),
  );

  function isProtected(net: NetworkSummary): boolean {
    return net.in_use || BUILTIN_NETWORKS.has(net.name);
  }

  function confirmDelete(net: NetworkSummary) {
    confirmState.request({
      title: `Delete network "${net.name}"?`,
      message: `Network "${net.name}" (${net.driver}) will be permanently removed.`,
      label: i18n.t.common.delete,
      danger: true,
      action: () => engineStore.removeNetwork(net.id, net.name),
    });
  }

  function confirmPrune() {
    confirmState.request({
      title: "Prune unused networks?",
      message: "All custom networks not currently used by any container will be removed.",
      label: "Prune Unused",
      danger: true,
      action: () => engineStore.pruneNetworks(),
    });
  }

  async function handleCreate(e: Event) {
    e.preventDefault();
    const name = newName.trim();
    if (!name) return;
    const ok = await engineStore.createNetwork(name, newDriver.trim() || "bridge", newInternal);
    if (ok) {
      newName = "";
      newDriver = "bridge";
      newInternal = false;
      createModalOpen = false;
    }
  }
</script>

<div class="mx-auto max-w-6xl space-y-5">
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <h1 class="text-xl font-bold tracking-tight text-slate-900 dark:text-white">
        {i18n.t.nav.networks}
      </h1>
      <p class="mt-0.5 text-xs text-slate-500 dark:text-slate-400">
        {totalCount} networks · {inUseCount} in use · {unusedCount} unused
      </p>
    </div>

    {#if engineStore.state !== "stopped"}
      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onclick={() => (createModalOpen = true)}
          disabled={engineStore.busy}
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3 py-1.5 text-xs font-medium text-white shadow-xs transition hover:bg-sky-500 disabled:opacity-50"
        >
          <Icon name="plus" class="h-3.5 w-3.5" />
          New Network
        </button>
        <button
          type="button"
          onclick={confirmPrune}
          disabled={engineStore.busy || unusedCount === 0}
          class="inline-flex items-center gap-1.5 rounded-lg border border-rose-200 bg-rose-50 px-3 py-1.5 text-xs font-medium text-rose-700 transition hover:bg-rose-100 disabled:opacity-40 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-300 dark:hover:bg-rose-500/20"
        >
          <Icon
            name={engineStore.isPending("networks:prune") ? "spinner" : "trash"}
            class={`h-3.5 w-3.5 ${engineStore.isPending("networks:prune") ? "animate-spin" : ""}`}
          />
          Prune Unused
        </button>
        <RefreshButton
          onRefresh={() => engineStore.refreshNetworks()}
          disabled={engineStore.loadingResources}
        />
      </div>
    {/if}
  </div>

  {#if engineStore.state === "stopped"}
    <EngineSleepingCard resourceLabel="Docker networks" />
  {:else}
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="inline-flex rounded-lg border border-slate-200 bg-white p-0.5 dark:border-slate-800 dark:bg-slate-900">
        {#each ["all", "in_use", "unused"] as const as tab}
          <button
            type="button"
            onclick={() => (usageFilter = tab)}
            class={`rounded-md px-3 py-1 text-xs font-medium transition ${
              usageFilter === tab
                ? "bg-slate-900 text-white dark:bg-slate-700"
                : "text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200"
            }`}
          >
            {tab === "all"
              ? `${i18n.t.common.all} (${totalCount})`
              : tab === "in_use"
                ? `${i18n.t.common.inUse} (${inUseCount})`
                : `${i18n.t.common.unused} (${unusedCount})`}
          </button>
        {/each}
      </div>
      <SearchInput bind:value={query} placeholder="Search networks..." />
    </div>

    <div class="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
      {#if filtered.length === 0}
        <div class="px-6 py-14 text-center text-sm text-slate-500 dark:text-slate-400">
          {totalCount === 0 ? "No Docker networks found." : "No networks match the current filter."}
        </div>
      {:else}
        <table class="w-full text-left text-sm">
          <thead class="border-b border-slate-200 bg-slate-50/70 text-[11px] font-semibold tracking-wider text-slate-500 uppercase dark:border-slate-800 dark:bg-slate-900 dark:text-slate-400">
            <tr>
              <th class="px-4 py-3">Name</th>
              <th class="px-4 py-3">Driver / Scope</th>
              <th class="px-4 py-3">Status</th>
              <th class="px-4 py-3 text-right">Actions</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-200 dark:divide-slate-800">
            {#each filtered as net (net.id)}
              <tr class="transition hover:bg-slate-50/70 dark:hover:bg-slate-800/40">
                <td class="px-4 py-3">
                  <div class="flex items-center gap-2">
                    <span class="font-medium text-slate-900 dark:text-white">{net.name}</span>
                    {#if net.internal}
                      <span class="rounded bg-amber-100 px-1.5 py-0.5 text-[10px] font-medium text-amber-800 dark:bg-amber-500/20 dark:text-amber-300">
                        internal
                      </span>
                    {/if}
                  </div>
                  <div class="mt-0.5 font-mono text-[11px] text-slate-400">{net.id}</div>
                </td>
                <td class="px-4 py-3 text-xs text-slate-600 dark:text-slate-300">
                  {net.driver} · {net.scope}
                </td>
                <td class="px-4 py-3">
                  <InUseBadge inUse={net.in_use} usedBy={net.used_by} />
                </td>
                <td class="px-4 py-3 text-right">
                  <button
                    type="button"
                    onclick={() => confirmDelete(net)}
                    disabled={engineStore.busy || isProtected(net)}
                    title={BUILTIN_NETWORKS.has(net.name)
                      ? "Built-in network cannot be deleted"
                      : net.in_use
                        ? "In use by containers"
                        : i18n.t.common.delete}
                    class="inline-flex items-center gap-1 rounded-md border border-slate-200 px-2.5 py-1 text-xs font-medium text-rose-600 transition hover:bg-rose-50 disabled:cursor-not-allowed disabled:opacity-40 dark:border-slate-700 dark:text-rose-400 dark:hover:bg-rose-500/10"
                  >
                    <Icon
                      name={engineStore.isPending(`network:${net.id}:remove`) ? "spinner" : "trash"}
                      class={`h-3.5 w-3.5 ${engineStore.isPending(`network:${net.id}:remove`) ? "animate-spin" : ""}`}
                    />
                    {i18n.t.common.delete}
                  </button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>
  {/if}
</div>

{#if createModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/60 p-4 backdrop-blur-xs">
    <form
      onsubmit={handleCreate}
      class="w-full max-w-md space-y-4 rounded-2xl border border-slate-200 bg-white p-5 shadow-2xl dark:border-slate-800 dark:bg-slate-900"
    >
      <div class="flex items-center justify-between">
        <h2 class="text-base font-bold text-slate-900 dark:text-white">Create Docker Network</h2>
        <button
          type="button"
          onclick={() => (createModalOpen = false)}
          class="rounded-md p-1 text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-800"
        >
          <Icon name="close" class="h-4 w-4" />
        </button>
      </div>

      <div class="space-y-3 text-xs">
        <label class="block space-y-1">
          <span class="font-medium text-slate-700 dark:text-slate-300">Network Name</span>
          <input
            type="text"
            required
            bind:value={newName}
            placeholder="e.g. app-network"
            class="w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 outline-none focus:border-sky-500 dark:border-slate-700 dark:bg-slate-950 dark:text-white"
          />
        </label>

        <label class="block space-y-1">
          <span class="font-medium text-slate-700 dark:text-slate-300">Driver</span>
          <select
            bind:value={newDriver}
            class="w-full rounded-lg border border-slate-300 bg-white px-3 py-2 text-sm text-slate-900 outline-none focus:border-sky-500 dark:border-slate-700 dark:bg-slate-950 dark:text-white"
          >
            <option value="bridge">bridge</option>
            <option value="overlay">overlay</option>
            <option value="macvlan">macvlan</option>
          </select>
        </label>

        <label class="flex items-center gap-2 pt-1 text-slate-700 dark:text-slate-300">
          <input type="checkbox" bind:checked={newInternal} class="h-4 w-4 accent-sky-600" />
          <span>Internal network (restrict external internet access)</span>
        </label>
      </div>

      <div class="flex justify-end gap-2 pt-2">
        <button
          type="button"
          onclick={() => (createModalOpen = false)}
          class="rounded-lg border border-slate-200 px-3 py-1.5 text-xs font-medium text-slate-600 hover:bg-slate-50 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
        >
          {i18n.t.common.cancel}
        </button>
        <button
          type="submit"
          disabled={!newName.trim() || engineStore.isPending("networks:create")}
          class="rounded-lg bg-sky-600 px-3.5 py-1.5 text-xs font-medium text-white hover:bg-sky-500 disabled:opacity-50"
        >
          {engineStore.isPending("networks:create") ? i18n.t.modals.creating : "Create"}
        </button>
      </div>
    </form>
  </div>
{/if}

<ConfirmModal
  open={confirmState.open}
  title={confirmState.title}
  message={confirmState.message}
  confirmLabel={confirmState.confirmLabel}
  danger={confirmState.danger}
  onConfirm={confirmState.execute}
  onCancel={confirmState.cancel}
/>
