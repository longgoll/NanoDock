<script lang="ts">
  import { onMount } from "svelte";
  import type { VolumeSummary } from "../../lib/api/daemon";
  import ConfirmModal from "../../lib/components/ConfirmModal.svelte";
  import EngineSleepingCard from "../../lib/components/EngineSleepingCard.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import InUseBadge from "../../lib/components/InUseBadge.svelte";
  import SearchInput from "../../lib/components/SearchInput.svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";
  import { useConfirm } from "../../lib/utils/confirm.svelte";
  import { formatIsoDate } from "../../lib/utils/format";

  type UsageFilter = "all" | "in_use" | "unused";

  let query = $state("");
  let usageFilter = $state<UsageFilter>("all");
  let selectedNames = $state<Set<string>>(new Set());
  let copiedMount = $state<string | null>(null);

  const confirmState = useConfirm();

  // Create volume modal
  let createModalOpen = $state(false);
  let newVolumeName = $state("");
  let newVolumeDriver = $state("local");
  let volumeNameInput = $state<HTMLInputElement | null>(null);

  onMount(() => {
    void engineStore.refreshVolumes(engineStore.volumes.length > 0);
  });

  const totalCount = $derived(engineStore.volumes.length);
  const inUseCount = $derived(engineStore.volumes.filter((v) => v.in_use).length);
  const unusedCount = $derived(totalCount - inUseCount);

  const filtered = $derived(
    engineStore.volumes.filter((v) => {
      if (usageFilter === "in_use" && !v.in_use) return false;
      if (usageFilter === "unused" && v.in_use) return false;

      const q = query.trim().toLowerCase();
      if (!q) return true;
      return (
        v.name.toLowerCase().includes(q) ||
        v.driver.toLowerCase().includes(q) ||
        v.mountpoint.toLowerCase().includes(q) ||
        v.used_by.some((c) => c.toLowerCase().includes(q))
      );
    }),
  );

  const selectedVolumes = $derived(
    engineStore.volumes.filter((v) => !v.in_use && selectedNames.has(v.name)),
  );
  const deletableVisible = $derived(filtered.filter((v) => !v.in_use));
  const allDeletableSelected = $derived(
    deletableVisible.length > 0 && deletableVisible.every((v) => selectedNames.has(v.name)),
  );

  function toggleSelectOne(vol: VolumeSummary) {
    if (vol.in_use) return;
    const next = new Set(selectedNames);
    if (next.has(vol.name)) {
      next.delete(vol.name);
    } else {
      next.add(vol.name);
    }
    selectedNames = next;
  }

  function toggleSelectAllUnused() {
    const next = new Set(selectedNames);
    if (allDeletableSelected) {
      for (const v of deletableVisible) next.delete(v.name);
    } else {
      for (const v of deletableVisible) next.add(v.name);
    }
    selectedNames = next;
  }

  function confirmDeleteSingle(vol: VolumeSummary) {
    confirmState.request({
      title: `Delete volume "${vol.name}"?`,
      message: `All persistent data stored in "${vol.name}" will be permanently deleted.`,
      label: "Delete Volume",
      danger: true,
      action: async () => {
        await engineStore.removeVolume(vol.name);
        const next = new Set(selectedNames);
        next.delete(vol.name);
        selectedNames = next;
      },
    });
  }

  function confirmDeleteSelected() {
    const names = Array.from(selectedNames);
    if (names.length === 0) return;
    confirmState.request({
      title: `Delete ${names.length} unused volume(s)?`,
      message: `All data in the ${names.length} selected unused volume(s) will be permanently lost.`,
      label: "Delete Selected",
      danger: true,
      action: async () => {
        await engineStore.removeVolumesBatch(names);
        selectedNames = new Set();
      },
    });
  }

  function confirmPruneUnused() {
    confirmState.request({
      title: `Prune ${unusedCount} unused volume(s)?`,
      message: "All volumes not currently mounted by any container will be permanently removed.",
      label: "Prune Unused",
      danger: false,
      action: async () => {
        await engineStore.pruneVolumes();
        selectedNames = new Set();
      },
    });
  }

  async function copyPath(path: string) {
    try {
      await navigator.clipboard.writeText(path);
      copiedMount = path;
      setTimeout(() => {
        if (copiedMount === path) copiedMount = null;
      }, 1400);
    } catch {
      // ignore
    }
  }

  function openCreateModal() {
    newVolumeName = "";
    newVolumeDriver = "local";
    createModalOpen = true;
    queueMicrotask(() => volumeNameInput?.focus());
  }

  async function handleCreateVolume(e: SubmitEvent) {
    e.preventDefault();
    const name = newVolumeName.trim();
    if (!name) return;
    const ok = await engineStore.createVolume(name, newVolumeDriver.trim() || "local");
    if (ok) {
      newVolumeName = "";
      createModalOpen = false;
    }
  }

  function handleWindowKeydown(e: KeyboardEvent) {
    if (createModalOpen && e.key === "Escape" && !engineStore.busy) {
      createModalOpen = false;
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget && !engineStore.busy) {
      createModalOpen = false;
    }
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<section class="space-y-4">
  <!-- Header -->
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <h2 class="text-lg font-semibold text-slate-900 dark:text-white">Volumes</h2>
      <p class="text-xs text-slate-500 dark:text-slate-400">
        {totalCount} total · {inUseCount} mounted (protected) · {unusedCount} unused
      </p>
    </div>

    {#if engineStore.state !== "stopped"}
      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onclick={openCreateModal}
          disabled={engineStore.busy}
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3 py-1.5 text-xs font-medium text-white transition hover:bg-sky-500 disabled:opacity-50"
        >
          <Icon name="plus" class="h-3.5 w-3.5" />
          Create Volume
        </button>
        {#if unusedCount > 0}
          <button
            type="button"
            onclick={confirmPruneUnused}
            disabled={engineStore.busy}
            class="inline-flex items-center gap-1.5 rounded-lg border border-amber-200 bg-amber-50 px-3 py-1.5 text-xs font-medium text-amber-700 transition hover:bg-amber-100 disabled:opacity-50 dark:border-amber-500/30 dark:bg-amber-500/15 dark:text-amber-300"
          >
            <Icon name="trash" class="h-3.5 w-3.5" />
            Prune Unused ({unusedCount})
          </button>
        {/if}
        <button
          type="button"
          onclick={() => engineStore.refreshVolumes()}
          disabled={engineStore.loadingResources}
          class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
        >
          <Icon
            name="refresh"
            class={`h-3.5 w-3.5 ${engineStore.loadingResources ? "animate-spin" : ""}`}
          />
          Refresh
        </button>
      </div>
    {/if}
  </div>

  {#if engineStore.state === "stopped"}
    <EngineSleepingCard resourceLabel="volumes" />
  {:else}
    <!-- Filter & Search Bar -->
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="inline-flex rounded-lg border border-slate-200 bg-white p-0.5 dark:border-slate-800 dark:bg-slate-900">
        <button
          type="button"
          onclick={() => (usageFilter = "all")}
          class={`rounded-md px-3 py-1 text-xs font-medium transition ${
            usageFilter === "all"
              ? "bg-slate-900 text-white dark:bg-slate-700"
              : "text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200"
          }`}
        >
          All ({totalCount})
        </button>
        <button
          type="button"
          onclick={() => (usageFilter = "in_use")}
          class={`rounded-md px-3 py-1 text-xs font-medium transition ${
            usageFilter === "in_use"
              ? "bg-emerald-600 text-white"
              : "text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200"
          }`}
        >
          In Use ({inUseCount})
        </button>
        <button
          type="button"
          onclick={() => (usageFilter = "unused")}
          class={`rounded-md px-3 py-1 text-xs font-medium transition ${
            usageFilter === "unused"
              ? "bg-amber-600 text-white"
              : "text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200"
          }`}
        >
          Unused ({unusedCount})
        </button>
      </div>

      <SearchInput bind:value={query} placeholder="Filter by name, container..." />
    </div>

    <!-- Bulk Action Bar -->
    {#if selectedVolumes.length > 0}
      <div class="flex items-center justify-between rounded-xl border border-sky-200 bg-sky-50/90 px-4 py-2.5 dark:border-sky-500/30 dark:bg-sky-500/10">
        <span class="text-xs font-semibold text-sky-800 dark:text-sky-300">
          {selectedVolumes.length} unused volume{selectedVolumes.length === 1 ? "" : "s"} selected
        </span>
        <div class="flex items-center gap-2">
          <button
            type="button"
            onclick={confirmDeleteSelected}
            disabled={engineStore.busy}
            class="rounded-md bg-rose-600 px-3 py-1 text-xs font-medium text-white transition hover:bg-rose-500 disabled:opacity-50"
          >
            Delete Selected
          </button>
          <button
            type="button"
            onclick={() => (selectedNames = new Set())}
            class="rounded-md px-2 py-1 text-xs font-medium text-slate-600 hover:bg-white/70 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            Clear
          </button>
        </div>
      </div>
    {/if}

    <!-- Volumes Table -->
    {#if filtered.length === 0}
      <div class="rounded-2xl border border-dashed border-slate-300 bg-white p-10 text-center text-sm text-slate-500 dark:border-slate-800 dark:bg-slate-900/30 dark:text-slate-400">
        No volumes found. Create one above or run
        <code class="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-sky-700 dark:bg-slate-800 dark:text-sky-400">
          nanodock volume create mydata
        </code>.
      </div>
    {:else}
      <div class="overflow-x-auto rounded-2xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900/50">
        <table class="w-full text-left text-sm">
          <thead class="border-b border-slate-200 bg-slate-50 text-xs font-semibold uppercase tracking-wider text-slate-500 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-400">
            <tr>
              <th class="w-10 px-4 py-3">
                <input
                  type="checkbox"
                  checked={allDeletableSelected}
                  disabled={deletableVisible.length === 0}
                  onchange={toggleSelectAllUnused}
                  title="Select all unused volumes"
                  aria-label="Select all unused volumes"
                  class="h-4 w-4 rounded border-slate-300 accent-sky-600 disabled:opacity-40"
                />
              </th>
              <th class="px-3 py-3">Name</th>
              <th class="px-3 py-3">Status</th>
              <th class="px-3 py-3">Driver</th>
              <th class="px-3 py-3">Mountpoint</th>
              <th class="px-3 py-3">Created</th>
              <th class="px-4 py-3 text-right">Actions</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-200 dark:divide-slate-800">
            {#each filtered as vol (vol.name)}
              <tr class="transition hover:bg-slate-50/80 dark:hover:bg-slate-800/40">
                <td class="px-4 py-3">
                  <input
                    type="checkbox"
                    checked={selectedNames.has(vol.name)}
                    disabled={vol.in_use}
                    onchange={() => toggleSelectOne(vol)}
                    title={vol.in_use ? `Protected: mounted by ${vol.used_by.join(", ")}` : "Select volume"}
                    aria-label={`Select volume ${vol.name}`}
                    class="h-4 w-4 rounded border-slate-300 accent-sky-600 disabled:cursor-not-allowed disabled:opacity-35"
                  />
                </td>
                <td
                  class="max-w-[200px] truncate px-3 py-3 font-medium text-slate-900 dark:text-white"
                  title={vol.name}
                >
                  {vol.name}
                </td>
                <td class="px-3 py-3">
                  <InUseBadge inUse={vol.in_use} usedBy={vol.used_by} verb="Mounted by" />
                </td>
                <td class="px-3 py-3">
                  <span class="rounded-md bg-slate-100 px-2 py-0.5 text-xs font-medium text-slate-700 dark:bg-slate-800 dark:text-slate-300">
                    {vol.driver}
                  </span>
                </td>
                <td class="px-3 py-3">
                  <button
                    type="button"
                    onclick={() => copyPath(vol.mountpoint)}
                    title="Click to copy mountpoint"
                    class="max-w-[210px] truncate text-left font-mono text-xs text-slate-500 hover:text-sky-600 dark:text-slate-400 dark:hover:text-sky-400"
                  >
                    {copiedMount === vol.mountpoint ? "Copied!" : vol.mountpoint}
                  </button>
                </td>
                <td class="px-3 py-3 text-xs text-slate-500 dark:text-slate-400">
                  {formatIsoDate(vol.created_at)}
                </td>
                <td class="px-4 py-3 text-right">
                  {#if vol.in_use}
                    <button
                      type="button"
                      disabled
                      title={`Cannot delete: mounted by ${vol.used_by.join(", ")}`}
                      aria-label={`Volume ${vol.name} is in use`}
                      class="inline-flex cursor-not-allowed items-center gap-1 rounded-lg border border-slate-200 px-2.5 py-1 text-xs font-medium text-slate-400 opacity-60 dark:border-slate-800 dark:text-slate-500"
                    >
                      <Icon name="lock" class="h-3.5 w-3.5" />
                      In Use
                    </button>
                  {:else}
                    <button
                      type="button"
                      onclick={() => confirmDeleteSingle(vol)}
                      disabled={engineStore.busy}
                      title="Delete unused volume"
                      aria-label={`Delete volume ${vol.name}`}
                      class="rounded-lg p-1.5 text-rose-600 transition hover:bg-rose-50 disabled:opacity-40 dark:text-rose-400 dark:hover:bg-rose-500/15"
                    >
                      <Icon name="trash" class="h-4 w-4" />
                    </button>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/if}
</section>

{#if createModalOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
  <div
    onclick={handleBackdropClick}
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/60 p-4 backdrop-blur-xs"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    aria-label="Create Volume"
  >
    <form
      onsubmit={handleCreateVolume}
      class="w-full max-w-sm space-y-4 rounded-2xl border border-slate-200 bg-white p-6 shadow-2xl dark:border-slate-800 dark:bg-slate-900"
    >
      <h3 class="text-base font-semibold text-slate-900 dark:text-white">Create Volume</h3>

      <div>
        <label for="vol-name" class="block text-xs font-medium text-slate-700 dark:text-slate-300">
          Volume Name
        </label>
        <input
          id="vol-name"
          bind:this={volumeNameInput}
          type="text"
          required
          bind:value={newVolumeName}
          placeholder="e.g. pg_data"
          class="mt-1 w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 text-sm text-slate-900 focus:border-sky-500 focus:bg-white focus:outline-none dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100"
        />
      </div>

      <div>
        <label for="vol-driver" class="block text-xs font-medium text-slate-700 dark:text-slate-300">
          Driver
        </label>
        <input
          id="vol-driver"
          type="text"
          bind:value={newVolumeDriver}
          class="mt-1 w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 font-mono text-xs text-slate-900 focus:border-sky-500 focus:bg-white focus:outline-none dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100"
        />
      </div>

      <div class="flex justify-end gap-2 pt-2">
        <button
          type="button"
          onclick={() => (createModalOpen = false)}
          disabled={engineStore.busy}
          class="rounded-lg border border-slate-200 bg-white px-4 py-2 text-xs font-medium text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={engineStore.busy || !newVolumeName.trim()}
          class="rounded-lg bg-sky-600 px-4 py-2 text-xs font-medium text-white hover:bg-sky-500 disabled:opacity-50"
        >
          {engineStore.busy ? "Creating..." : "Create"}
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
  busy={engineStore.busy}
  onConfirm={confirmState.execute}
  onCancel={confirmState.cancel}
/>
