<script lang="ts">
  import { onMount } from "svelte";
  import type { ImageSummary } from "../../lib/api/daemon";
  import ConfirmModal from "../../lib/components/ConfirmModal.svelte";
  import EngineSleepingCard from "../../lib/components/EngineSleepingCard.svelte";
  import HelpTip from "../../lib/components/HelpTip.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import InUseBadge from "../../lib/components/InUseBadge.svelte";
  import RunContainerModal from "../../lib/components/RunContainerModal.svelte";
  import SearchInput from "../../lib/components/SearchInput.svelte";
  import Tooltip from "../../lib/components/Tooltip.svelte";
  import { i18n } from "../../lib/i18n/index.svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";
  import { useConfirm } from "../../lib/utils/confirm.svelte";
  import { formatBytes, formatTimestamp } from "../../lib/utils/format";

  type UsageFilter = "all" | "in_use" | "unused";

  let query = $state("");
  let pullRef = $state("");
  let usageFilter = $state<UsageFilter>("all");
  let selectedIds = $state<Set<string>>(new Set());
  let copiedId = $state<string | null>(null);

  const confirmState = useConfirm();

  // Run container modal state
  let runModalOpen = $state(false);
  let runImageRef = $state("");

  onMount(() => {
    void engineStore.refreshImages(engineStore.images.length > 0);
  });

  const totalCount = $derived(engineStore.images.length);
  const inUseCount = $derived(engineStore.images.filter((i) => i.in_use).length);
  const unusedCount = $derived(totalCount - inUseCount);
  const totalSize = $derived(engineStore.images.reduce((sum, i) => sum + i.size, 0));
  const reclaimableSize = $derived(
    engineStore.images.filter((i) => !i.in_use).reduce((sum, i) => sum + i.size, 0),
  );

  const filtered = $derived(
    engineStore.images.filter((img) => {
      if (usageFilter === "in_use" && !img.in_use) return false;
      if (usageFilter === "unused" && img.in_use) return false;

      const q = query.trim().toLowerCase();
      if (!q) return true;
      return (
        img.id.toLowerCase().includes(q) ||
        img.repo_tags.some((t) => t.toLowerCase().includes(q)) ||
        img.used_by.some((c) => c.toLowerCase().includes(q))
      );
    }),
  );

  const selectedImages = $derived(
    engineStore.images.filter((img) => !img.in_use && selectedIds.has(img.id)),
  );
  const deletableVisible = $derived(filtered.filter((img) => !img.in_use));
  const allDeletableSelected = $derived(
    deletableVisible.length > 0 && deletableVisible.every((img) => selectedIds.has(img.id)),
  );

  function toggleSelectOne(img: ImageSummary) {
    if (img.in_use) return;
    const next = new Set(selectedIds);
    if (next.has(img.id)) {
      next.delete(img.id);
    } else {
      next.add(img.id);
    }
    selectedIds = next;
  }

  function toggleSelectAllUnused() {
    const next = new Set(selectedIds);
    if (allDeletableSelected) {
      for (const img of deletableVisible) next.delete(img.id);
    } else {
      for (const img of deletableVisible) next.add(img.id);
    }
    selectedIds = next;
  }

  function confirmDeleteSingle(img: ImageSummary) {
    const label = img.repo_tags[0] || img.id;
    confirmState.request({
      title: `Delete image "${label}"?`,
      message: `This will remove image ${img.id} (${formatBytes(img.size)}) from local storage.`,
      label: "Delete Image",
      danger: true,
      action: async () => {
        await engineStore.removeImage(img.id);
        const next = new Set(selectedIds);
        next.delete(img.id);
        selectedIds = next;
      },
    });
  }

  function confirmDeleteSelected() {
    const ids = Array.from(selectedIds);
    if (ids.length === 0) return;
    confirmState.request({
      title: `Delete ${ids.length} unused image(s)?`,
      message: `This will permanently remove ${ids.length} selected unused image(s).`,
      label: "Delete Selected",
      danger: true,
      action: async () => {
        await engineStore.removeImagesBatch(ids);
        selectedIds = new Set();
      },
    });
  }

  function confirmPruneUnused() {
    confirmState.request({
      title: "Prune all unused images?",
      message: `Up to ${formatBytes(reclaimableSize)} across ${unusedCount} unused image(s) may be reclaimed. Images currently used by containers will be kept.`,
      label: "Prune Unused",
      danger: false,
      action: async () => {
        await engineStore.pruneImages();
        selectedIds = new Set();
      },
    });
  }

  async function handlePull(e: SubmitEvent) {
    e.preventDefault();
    const ref = pullRef.trim();
    if (!ref) return;
    const ok = await engineStore.pullImage(ref);
    if (ok) {
      pullRef = "";
    }
  }

  function openRunModal(img: ImageSummary) {
    runImageRef =
      img.repo_tags.length > 0 && img.repo_tags[0] !== "<none>:<none>"
        ? img.repo_tags[0]
        : img.id;
    runModalOpen = true;
  }

  async function copyImageId(id: string) {
    try {
      await navigator.clipboard.writeText(id);
      copiedId = id;
      setTimeout(() => {
        if (copiedId === id) copiedId = null;
      }, 1400);
    } catch {
      // ignore
    }
  }
</script>

<section class="space-y-4">
  <!-- Header -->
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <div class="flex items-center gap-2">
        <h2 class="text-lg font-semibold text-slate-900 dark:text-white">Images</h2>
        <HelpTip
          title="Image Protection &amp; Dangling Layers"
          align="left"
          position="bottom"
          text="Images marked In Use are referenced by at least one container (including stopped ones) and cannot be deleted. <dangling> tags are leftover build layers safe to prune."
        />
      </div>
      <p class="text-xs text-slate-500 dark:text-slate-400">
        {totalCount} total ({formatBytes(totalSize)}) · {inUseCount} in use · {unusedCount} unused ({formatBytes(reclaimableSize)} reclaimable)
      </p>
    </div>

    {#if engineStore.state !== "stopped"}
      <div class="flex flex-wrap items-center gap-2">
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
          onclick={() => engineStore.refreshImages()}
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
    <EngineSleepingCard resourceLabel="images" />
  {:else}
    <!-- Pull Image Bar -->
    <form
      onsubmit={handlePull}
      class="flex items-center gap-2.5 rounded-2xl border border-slate-200 bg-white p-3 shadow-xs dark:border-slate-800 dark:bg-slate-900/50"
    >
        <input
          type="text"
          bind:value={pullRef}
          aria-label="Pull image reference"
          placeholder={i18n.t.images.pullPlaceholder}
          class="flex-1 rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 text-sm text-slate-900 placeholder-slate-400 focus:border-sky-500 focus:bg-white focus:outline-none dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100"
        />
        <button
          type="submit"
          disabled={engineStore.pullingRef !== null || !pullRef.trim()}
          class="rounded-lg bg-sky-600 px-4 py-1.5 text-xs font-medium text-white transition hover:bg-sky-500 disabled:opacity-50"
        >
          {engineStore.pullingRef ? i18n.t.images.pulling : i18n.t.images.pullImage}
        </button>
    </form>

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

      <SearchInput bind:value={query} placeholder="Filter by tag, ID, container..." />
    </div>

    <!-- Bulk Action Bar -->
    {#if selectedImages.length > 0}
      <div class="flex items-center justify-between rounded-xl border border-sky-200 bg-sky-50/90 px-4 py-2.5 dark:border-sky-500/30 dark:bg-sky-500/10">
        <span class="text-xs font-semibold text-sky-800 dark:text-sky-300">
          {selectedImages.length} unused image{selectedImages.length === 1 ? "" : "s"} selected
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
            onclick={() => (selectedIds = new Set())}
            class="rounded-md px-2 py-1 text-xs font-medium text-slate-600 hover:bg-white/70 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            Clear
          </button>
        </div>
      </div>
    {/if}

    {#if engineStore.pullingRef}
      <div class="flex items-center gap-2.5 rounded-xl border border-sky-200 bg-sky-50 px-4 py-2.5 text-xs font-medium text-sky-800 dark:border-sky-500/30 dark:bg-sky-500/10 dark:text-sky-300">
        <Icon name="refresh" class="h-4 w-4 animate-spin" />
        <span>Pulling <span class="font-mono font-semibold">{engineStore.pullingRef}</span>...</span>
      </div>
    {/if}

    <!-- Images Table -->
    {#if filtered.length === 0}
      <div class="rounded-2xl border border-dashed border-slate-300 bg-white p-10 text-center text-sm text-slate-500 dark:border-slate-800 dark:bg-slate-900/30 dark:text-slate-400">
        No images found. Pull an image above or run
        <code class="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-sky-700 dark:bg-slate-800 dark:text-sky-400">
          nanodock pull alpine
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
                  title="Select all unused images"
                  aria-label="Select all unused images"
                  class="h-4 w-4 rounded border-slate-300 accent-sky-600 disabled:opacity-40"
                />
              </th>
              <th class="px-3 py-3">Repository : Tag</th>
              <th class="px-3 py-3">Status</th>
              <th class="px-3 py-3">Image ID</th>
              <th class="px-3 py-3">Size</th>
              <th class="px-3 py-3">Created</th>
              <th class="px-4 py-3 text-right">Actions</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-200 dark:divide-slate-800">
            {#each filtered as img (img.id)}
              <tr class="transition hover:bg-slate-50/80 dark:hover:bg-slate-800/40">
                <td class="px-4 py-3">
                  <input
                    type="checkbox"
                    checked={selectedIds.has(img.id)}
                    disabled={img.in_use}
                    onchange={() => toggleSelectOne(img)}
                    title={img.in_use ? `Protected: in use by ${img.used_by.join(", ")}` : "Select image"}
                    aria-label={`Select image ${img.repo_tags[0] || img.id}`}
                    class="h-4 w-4 rounded border-slate-300 accent-sky-600 disabled:cursor-not-allowed disabled:opacity-35"
                  />
                </td>
                <td class="px-3 py-3">
                  <div class="flex flex-wrap gap-1.5">
                    {#if img.repo_tags.length > 0 && img.repo_tags[0] !== "<none>:<none>"}
                      {#each img.repo_tags as tag}
                        <span class="rounded-md bg-slate-100 px-2 py-0.5 font-mono text-xs font-medium text-slate-800 dark:bg-slate-800 dark:text-slate-200">
                          {tag}
                        </span>
                      {/each}
                    {:else}
                      <span class="rounded-md bg-amber-50 px-2 py-0.5 font-mono text-xs text-amber-700 dark:bg-amber-500/15 dark:text-amber-300">
                        &lt;dangling&gt;
                      </span>
                    {/if}
                  </div>
                </td>
                <td class="px-3 py-3">
                  <InUseBadge inUse={img.in_use} usedBy={img.used_by} verb="Used by" />
                </td>
                <td class="px-3 py-3">
                  <button
                    type="button"
                    onclick={() => copyImageId(img.id)}
                    title="Click to copy Image ID"
                    class="font-mono text-xs text-slate-500 hover:text-sky-600 dark:text-slate-400 dark:hover:text-sky-400"
                  >
                    {copiedId === img.id ? "Copied!" : img.id}
                  </button>
                </td>
                <td class="px-3 py-3 text-xs font-medium text-slate-700 dark:text-slate-300">
                  {formatBytes(img.size)}
                </td>
                <td class="px-3 py-3 text-xs text-slate-500 dark:text-slate-400">
                  {formatTimestamp(img.created)}
                </td>
                <td class="px-4 py-3 text-right">
                  <div class="inline-flex items-center gap-1">
                    <Tooltip text={i18n.t.images.runTooltip}>
                      <button
                        type="button"
                        onclick={() => openRunModal(img)}
                        disabled={engineStore.busy}
                        aria-label={`Run container from ${img.repo_tags[0] || img.id}`}
                        class="rounded-lg p-1.5 text-emerald-600 transition hover:bg-emerald-50 disabled:opacity-40 dark:text-emerald-400 dark:hover:bg-emerald-500/15"
                      >
                        <Icon name="play" class="h-4 w-4" />
                      </button>
                    </Tooltip>
                    {#if img.in_use}
                      <Tooltip text={i18n.t.images.lockedTooltip(img.used_by.join(", "))} align="right">
                        <button
                          type="button"
                          disabled
                          aria-label={`Image ${img.repo_tags[0] || img.id} is in use`}
                          class="inline-flex cursor-not-allowed items-center gap-1 rounded-lg border border-slate-200 px-2 py-1 text-xs font-medium text-slate-400 opacity-60 dark:border-slate-800 dark:text-slate-500"
                        >
                          <Icon name="lock" class="h-3.5 w-3.5" />
                          {i18n.t.common.inUse}
                        </button>
                      </Tooltip>
                    {:else}
                      <Tooltip text={i18n.t.images.deleteTooltip} align="right">
                        <button
                          type="button"
                          onclick={() => confirmDeleteSingle(img)}
                          disabled={engineStore.busy}
                          aria-label={`Delete image ${img.repo_tags[0] || img.id}`}
                          class="rounded-lg p-1.5 text-rose-600 transition hover:bg-rose-50 disabled:opacity-40 dark:text-rose-400 dark:hover:bg-rose-500/15"
                        >
                          <Icon name="trash" class="h-4 w-4" />
                        </button>
                      </Tooltip>
                    {/if}
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/if}
</section>

<RunContainerModal
  open={runModalOpen}
  initialImage={runImageRef}
  onClose={() => (runModalOpen = false)}
/>

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
