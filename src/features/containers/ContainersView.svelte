<script lang="ts">
  import { onMount } from "svelte";
  import {
    daemonApi,
    type ContainerActionKind,
    type ContainerSummary,
  } from "../../lib/api/daemon";
  import ConfirmModal from "../../lib/components/ConfirmModal.svelte";
  import EngineSleepingCard from "../../lib/components/EngineSleepingCard.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import RunContainerModal from "../../lib/components/RunContainerModal.svelte";
  import SearchInput from "../../lib/components/SearchInput.svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";
  import { useConfirm } from "../../lib/utils/confirm.svelte";
  import ContainerDetailView, { type DetailTab } from "./ContainerDetailView.svelte";

  type StatusFilter = "all" | "running" | "stopped";

  interface ContainerGroup {
    name: string;
    items: ContainerSummary[];
    runningCount: number;
  }

  let query = $state("");
  let statusFilter = $state<StatusFilter>("all");
  let groupByStack = $state(true);
  let collapsedGroups = $state<Record<string, boolean>>({});
  let selectedIds = $state<Set<string>>(new Set());
  let copiedId = $state<string | null>(null);

  const confirmState = useConfirm();

  // Detail view state (Master-Detail)
  let detailContainerId = $state<string | null>(null);
  let detailInitialTab = $state<DetailTab>("logs");

  // Run container modal state
  let runModalOpen = $state(false);

  onMount(() => {
    void engineStore.refreshContainers(engineStore.containers.length > 0);
  });

  const totalCount = $derived(engineStore.containers.length);
  const runningCount = $derived(
    engineStore.containers.filter((c) => c.state === "running").length,
  );
  const stoppedCount = $derived(totalCount - runningCount);

  const filtered = $derived(
    engineStore.containers.filter((c) => {
      if (statusFilter === "running" && c.state !== "running") return false;
      if (statusFilter === "stopped" && c.state === "running") return false;

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

  const grouped = $derived.by(() => {
    const groupsMap = new Map<string, ContainerSummary[]>();
    const standalone: ContainerSummary[] = [];

    for (const c of filtered) {
      if (groupByStack && c.compose_project) {
        const list = groupsMap.get(c.compose_project) ?? [];
        list.push(c);
        groupsMap.set(c.compose_project, list);
      } else {
        standalone.push(c);
      }
    }

    const groups: ContainerGroup[] = Array.from(groupsMap.entries())
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([name, items]) => ({
        name,
        items,
        runningCount: items.filter((c) => c.state === "running").length,
      }));

    return { groups, standalone };
  });

  const allVisibleSelected = $derived(
    filtered.length > 0 && filtered.every((c) => selectedIds.has(c.id)),
  );

  const someVisibleSelected = $derived(
    !allVisibleSelected && filtered.some((c) => selectedIds.has(c.id)),
  );

  const selectedContainers = $derived(
    engineStore.containers.filter((c) => selectedIds.has(c.id)),
  );

  function toggleSelectAllVisible() {
    const next = new Set(selectedIds);
    if (allVisibleSelected) {
      for (const c of filtered) next.delete(c.id);
    } else {
      for (const c of filtered) next.add(c.id);
    }
    selectedIds = next;
  }

  function toggleSelectOne(id: string) {
    const next = new Set(selectedIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    selectedIds = next;
  }

  function isGroupAllSelected(group: ContainerGroup): boolean {
    return group.items.length > 0 && group.items.every((c) => selectedIds.has(c.id));
  }

  function isGroupSomeSelected(group: ContainerGroup): boolean {
    return !isGroupAllSelected(group) && group.items.some((c) => selectedIds.has(c.id));
  }

  function toggleSelectGroup(group: ContainerGroup) {
    const next = new Set(selectedIds);
    const allSelected = isGroupAllSelected(group);
    for (const c of group.items) {
      if (allSelected) {
        next.delete(c.id);
      } else {
        next.add(c.id);
      }
    }
    selectedIds = next;
  }

  function toggleCollapseGroup(name: string) {
    collapsedGroups = {
      ...collapsedGroups,
      [name]: !collapsedGroups[name],
    };
  }

  function setAllGroupsCollapsed(collapsed: boolean) {
    const next: Record<string, boolean> = {};
    for (const g of grouped.groups) {
      next[g.name] = collapsed;
    }
    collapsedGroups = next;
  }

  function filterTargetIds(items: ContainerSummary[], action: ContainerActionKind): string[] {
    if (action === "start") {
      return items.filter((c) => c.state !== "running").map((c) => c.id);
    }
    if (action === "stop" || action === "restart") {
      return items.filter((c) => c.state === "running").map((c) => c.id);
    }
    return items.map((c) => c.id);
  }

  async function runGroupAction(group: ContainerGroup, action: ContainerActionKind) {
    const targetIds = filterTargetIds(group.items, action);
    if (targetIds.length === 0) return;

    if (action === "remove") {
      confirmState.request({
        title: `Delete stack "${group.name}"?`,
        message: `This will force-remove all ${targetIds.length} container(s) in stack "${group.name}".`,
        label: "Delete Stack",
        danger: true,
        action: async () => {
          await engineStore.runContainersBatch(targetIds, "remove");
          const next = new Set(selectedIds);
          for (const id of targetIds) next.delete(id);
          selectedIds = next;
        },
      });
      return;
    }

    await engineStore.runContainersBatch(targetIds, action);
  }

  async function runSelectedAction(action: ContainerActionKind) {
    const targetIds = filterTargetIds(selectedContainers, action);
    if (targetIds.length === 0) return;

    if (action === "remove") {
      confirmState.request({
        title: `Delete ${targetIds.length} container(s)?`,
        message: `Are you sure you want to force-remove ${targetIds.length} selected container(s)? This cannot be undone.`,
        label: "Delete Selected",
        danger: true,
        action: async () => {
          await engineStore.runContainersBatch(targetIds, "remove");
          selectedIds = new Set();
        },
      });
      return;
    }

    await engineStore.runContainersBatch(targetIds, action);
  }

  function stopAllRunning() {
    const ids = engineStore.containers
      .filter((c) => c.state === "running")
      .map((c) => c.id);
    if (ids.length === 0) return;
    confirmState.request({
      title: `Stop all ${ids.length} running container(s)?`,
      message: "All currently running containers will be gracefully stopped.",
      label: "Stop All",
      danger: false,
      action: async () => {
        await engineStore.runContainersBatch(ids, "stop");
      },
    });
  }

  function confirmDeleteSingle(c: ContainerSummary) {
    const displayName = c.names[0] || c.id;
    confirmState.request({
      title: `Delete container "${displayName}"?`,
      message: `This will force-remove "${displayName}" (${c.id}).`,
      label: "Delete",
      danger: true,
      action: async () => {
        await engineStore.runContainerAction(c.id, "remove");
        const next = new Set(selectedIds);
        next.delete(c.id);
        selectedIds = next;
      },
    });
  }

  function openDetail(id: string, tab: DetailTab = "logs") {
    detailInitialTab = tab;
    detailContainerId = id;
  }

  async function openShell(id: string) {
    try {
      await daemonApi.openContainerShell(id);
    } catch (e) {
      engineStore.error = String(e);
    }
  }

  function extractHostPort(portText: string): number | null {
    const colon = portText.indexOf(":");
    if (colon <= 0) return null;
    const n = Number.parseInt(portText.slice(0, colon), 10);
    return Number.isFinite(n) && n > 0 ? n : null;
  }

  async function openPort(portText: string) {
    const hostPort = extractHostPort(portText);
    if (!hostPort) return;
    try {
      await daemonApi.openLocalhostPort(hostPort);
    } catch (e) {
      engineStore.error = String(e);
    }
  }

  async function copyContainerId(id: string) {
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

{#if detailContainerId}
  <ContainerDetailView
    containerId={detailContainerId}
    initialTab={detailInitialTab}
    onBack={() => (detailContainerId = null)}
    onDeleted={() => {
      detailContainerId = null;
    }}
  />
{:else}
<section class="space-y-4">
  <!-- Header -->
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <h2 class="text-lg font-semibold text-slate-900 dark:text-white">Containers</h2>
      <p class="text-xs text-slate-500 dark:text-slate-400">
        {runningCount} running · {stoppedCount} stopped · {totalCount} total
      </p>
    </div>

    {#if engineStore.state !== "stopped"}
      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onclick={() => (runModalOpen = true)}
          disabled={engineStore.busy}
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3 py-1.5 text-xs font-medium text-white transition hover:bg-sky-500 disabled:opacity-50"
        >
          <Icon name="plus" class="h-3.5 w-3.5" />
          Run Container
        </button>

        {#if runningCount > 0}
          <button
            type="button"
            onclick={stopAllRunning}
            disabled={engineStore.busy}
            class="inline-flex items-center gap-1.5 rounded-lg border border-rose-200 bg-rose-50 px-3 py-1.5 text-xs font-medium text-rose-700 transition hover:bg-rose-100 disabled:opacity-50 dark:border-rose-500/30 dark:bg-rose-500/15 dark:text-rose-300 dark:hover:bg-rose-500/25"
            title="Stop all running containers"
          >
            <Icon name="stop" class="h-3.5 w-3.5" />
            Stop All ({runningCount})
          </button>
        {/if}

        <button
          type="button"
          onclick={() => (groupByStack = !groupByStack)}
          class={`inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs font-medium transition ${
            groupByStack
              ? "border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-500/30 dark:bg-sky-500/15 dark:text-sky-300"
              : "border-slate-200 bg-white text-slate-600 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300"
          }`}
          title="Toggle Compose stack grouping"
        >
          <Icon name="images" class="h-3.5 w-3.5" />
          Group by Stack
        </button>

        <button
          type="button"
          onclick={() => engineStore.refreshContainers()}
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
    <EngineSleepingCard resourceLabel="containers" />
  {:else}
    <!-- Filter & Search Bar -->
    <div class="flex flex-wrap items-center justify-between gap-3">
      <div class="inline-flex rounded-lg border border-slate-200 bg-white p-0.5 dark:border-slate-800 dark:bg-slate-900">
        <button
          type="button"
          onclick={() => (statusFilter = "all")}
          class={`rounded-md px-3 py-1 text-xs font-medium transition ${
            statusFilter === "all"
              ? "bg-slate-900 text-white dark:bg-slate-700"
              : "text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200"
          }`}
        >
          All ({totalCount})
        </button>
        <button
          type="button"
          onclick={() => (statusFilter = "running")}
          class={`rounded-md px-3 py-1 text-xs font-medium transition ${
            statusFilter === "running"
              ? "bg-emerald-600 text-white"
              : "text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200"
          }`}
        >
          Running ({runningCount})
        </button>
        <button
          type="button"
          onclick={() => (statusFilter = "stopped")}
          class={`rounded-md px-3 py-1 text-xs font-medium transition ${
            statusFilter === "stopped"
              ? "bg-slate-700 text-white dark:bg-slate-600"
              : "text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-200"
          }`}
        >
          Stopped ({stoppedCount})
        </button>
      </div>

      <div class="flex items-center gap-2">
        {#if groupByStack && grouped.groups.length > 0}
          <button
            type="button"
            onclick={() => setAllGroupsCollapsed(false)}
            class="rounded-md px-2 py-1 text-xs font-medium text-slate-500 hover:bg-slate-100 hover:text-slate-800 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
          >
            Expand all
          </button>
          <button
            type="button"
            onclick={() => setAllGroupsCollapsed(true)}
            class="rounded-md px-2 py-1 text-xs font-medium text-slate-500 hover:bg-slate-100 hover:text-slate-800 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
          >
            Collapse all
          </button>
        {/if}

        <SearchInput bind:value={query} placeholder="Filter by name, image, stack..." />
      </div>
    </div>

    <!-- Bulk Action Bar when items selected -->
    {#if selectedContainers.length > 0}
      <div class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-sky-200 bg-sky-50/90 px-4 py-2.5 dark:border-sky-500/30 dark:bg-sky-500/10">
        <div class="flex items-center gap-2 text-xs font-semibold text-sky-800 dark:text-sky-300">
          <span>{selectedContainers.length} container{selectedContainers.length === 1 ? "" : "s"} selected</span>
        </div>
        <div class="flex flex-wrap items-center gap-1.5">
          <button
            type="button"
            onclick={() => runSelectedAction("start")}
            disabled={engineStore.busy || selectedContainers.every((c) => c.state === "running")}
            class="inline-flex items-center gap-1 rounded-md bg-emerald-600 px-2.5 py-1 text-xs font-medium text-white transition hover:bg-emerald-500 disabled:opacity-40"
          >
            <Icon name="play" class="h-3 w-3" />
            Start
          </button>
          <button
            type="button"
            onclick={() => runSelectedAction("stop")}
            disabled={engineStore.busy || selectedContainers.every((c) => c.state !== "running")}
            class="inline-flex items-center gap-1 rounded-md bg-amber-600 px-2.5 py-1 text-xs font-medium text-white transition hover:bg-amber-500 disabled:opacity-40"
          >
            <Icon name="stop" class="h-3 w-3" />
            Stop
          </button>
          <button
            type="button"
            onclick={() => runSelectedAction("restart")}
            disabled={engineStore.busy || selectedContainers.every((c) => c.state !== "running")}
            class="inline-flex items-center gap-1 rounded-md border border-slate-300 bg-white px-2.5 py-1 text-xs font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-40 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200"
          >
            Restart
          </button>
          <button
            type="button"
            onclick={() => runSelectedAction("remove")}
            disabled={engineStore.busy}
            class="inline-flex items-center gap-1 rounded-md bg-rose-600 px-2.5 py-1 text-xs font-medium text-white transition hover:bg-rose-500 disabled:opacity-40"
          >
            Delete
          </button>
          <button
            type="button"
            onclick={() => (selectedIds = new Set())}
            class="ml-1 rounded-md px-2 py-1 text-xs font-medium text-slate-600 hover:bg-white/70 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            Clear
          </button>
        </div>
      </div>
    {/if}

    <!-- Container Table -->
    {#if filtered.length === 0}
      <div class="rounded-2xl border border-dashed border-slate-300 bg-white p-10 text-center text-sm text-slate-500 dark:border-slate-800 dark:bg-slate-900/30 dark:text-slate-400">
        No containers match your filter. Click <strong>Run Container</strong> above or run
        <code class="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-sky-700 dark:bg-slate-800 dark:text-sky-400">
          nanodock run -d -p 8080:80 nginx:alpine
        </code>
        to get started.
      </div>
    {:else}
      <div class="overflow-x-auto rounded-2xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900/50">
        <table class="w-full text-left text-sm">
          <thead class="border-b border-slate-200 bg-slate-50 text-xs font-semibold uppercase tracking-wider text-slate-500 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-400">
            <tr>
              <th class="w-10 px-4 py-3">
                <input
                  type="checkbox"
                  checked={allVisibleSelected}
                  indeterminate={someVisibleSelected}
                  onchange={toggleSelectAllVisible}
                  aria-label="Select all containers"
                  class="h-4 w-4 rounded border-slate-300 accent-sky-600"
                />
              </th>
              <th class="px-3 py-3">Name</th>
              <th class="px-3 py-3">Image</th>
              <th class="px-3 py-3">Status</th>
              <th class="px-3 py-3">Ports</th>
              <th class="px-4 py-3 text-right">Actions</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-slate-200 dark:divide-slate-800">
            <!-- Compose Stack Groups -->
            {#each grouped.groups as group (group.name)}
              {@const isCollapsed = !!collapsedGroups[group.name]}
              {@const groupAllSelected = isGroupAllSelected(group)}
              {@const groupSomeSelected = isGroupSomeSelected(group)}
              <tr class="bg-slate-50/90 transition hover:bg-slate-100/80 dark:bg-slate-800/60 dark:hover:bg-slate-800">
                <td class="px-4 py-2.5">
                  <input
                    type="checkbox"
                    checked={groupAllSelected}
                    indeterminate={groupSomeSelected}
                    onchange={() => toggleSelectGroup(group)}
                    aria-label={`Select all in stack ${group.name}`}
                    class="h-4 w-4 rounded border-slate-300 accent-sky-600"
                  />
                </td>
                <td colspan="4" class="px-3 py-2.5">
                  <button
                    type="button"
                    onclick={() => toggleCollapseGroup(group.name)}
                    class="flex w-full items-center gap-2.5 text-left"
                  >
                    <Icon
                      name="chevron-down"
                      class={`h-4 w-4 shrink-0 text-slate-400 transition-transform ${
                        isCollapsed ? "-rotate-90" : "rotate-0"
                      }`}
                    />
                    <div class="flex h-6 w-6 items-center justify-center rounded-md bg-sky-100 text-sky-700 dark:bg-sky-500/20 dark:text-sky-400">
                      <Icon name="images" class="h-3.5 w-3.5" />
                    </div>
                    <span class="font-semibold text-slate-900 dark:text-white">{group.name}</span>
                    <span
                      class={`rounded-full px-2 py-0.5 text-[11px] font-medium ${
                        group.runningCount === group.items.length
                          ? "bg-emerald-100 text-emerald-700 dark:bg-emerald-500/20 dark:text-emerald-300"
                          : group.runningCount > 0
                            ? "bg-amber-100 text-amber-700 dark:bg-amber-500/20 dark:text-amber-300"
                            : "bg-slate-200 text-slate-600 dark:bg-slate-700 dark:text-slate-300"
                      }`}
                    >
                      {group.runningCount}/{group.items.length} running
                    </span>
                  </button>
                </td>
                <td class="px-4 py-2.5 text-right">
                  <div class="inline-flex items-center gap-1">
                    {#if group.runningCount < group.items.length}
                      <button
                        type="button"
                        onclick={() => runGroupAction(group, "start")}
                        disabled={engineStore.busy}
                        title={`Start all in ${group.name}`}
                        aria-label={`Start all in stack ${group.name}`}
                        class="rounded-lg p-1.5 text-emerald-600 transition hover:bg-emerald-50 disabled:opacity-40 dark:text-emerald-400 dark:hover:bg-emerald-500/15"
                      >
                        <Icon name="play" class="h-4 w-4" />
                      </button>
                    {/if}
                    {#if group.runningCount > 0}
                      <button
                        type="button"
                        onclick={() => runGroupAction(group, "stop")}
                        disabled={engineStore.busy}
                        title={`Stop all in ${group.name}`}
                        aria-label={`Stop all in stack ${group.name}`}
                        class="rounded-lg p-1.5 text-amber-600 transition hover:bg-amber-50 disabled:opacity-40 dark:text-amber-400 dark:hover:bg-amber-500/15"
                      >
                        <Icon name="stop" class="h-4 w-4" />
                      </button>
                      <button
                        type="button"
                        onclick={() => runGroupAction(group, "restart")}
                        disabled={engineStore.busy}
                        title={`Restart running in ${group.name}`}
                        aria-label={`Restart running in stack ${group.name}`}
                        class="rounded-lg p-1.5 text-sky-600 transition hover:bg-sky-50 disabled:opacity-40 dark:text-sky-400 dark:hover:bg-sky-500/15"
                      >
                        <Icon name="refresh" class="h-4 w-4" />
                      </button>
                    {/if}
                    <button
                      type="button"
                      onclick={() => runGroupAction(group, "remove")}
                      disabled={engineStore.busy}
                      title={`Delete stack ${group.name}`}
                      aria-label={`Delete stack ${group.name}`}
                      class="rounded-lg p-1.5 text-rose-600 transition hover:bg-rose-50 disabled:opacity-40 dark:text-rose-400 dark:hover:bg-rose-500/15"
                    >
                      <Icon name="trash" class="h-4 w-4" />
                    </button>
                  </div>
                </td>
              </tr>

              {#if !isCollapsed}
                {#each group.items as c (c.id)}
                  {@render containerRow(c, true)}
                {/each}
              {/if}
            {/each}

            <!-- Standalone Containers -->
            {#each grouped.standalone as c (c.id)}
              {@render containerRow(c, false)}
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/if}
</section>

{#snippet containerRow(c: ContainerSummary, indented: boolean)}
  {@const displayName = c.names[0] || c.id}
  <tr class="transition hover:bg-slate-50/80 dark:hover:bg-slate-800/40">
    <td class="px-4 py-3">
      <input
        type="checkbox"
        checked={selectedIds.has(c.id)}
        onchange={() => toggleSelectOne(c.id)}
        aria-label={`Select ${displayName}`}
        class="h-4 w-4 rounded border-slate-300 accent-sky-600"
      />
    </td>
    <td class={`py-3 pr-3 ${indented ? "pl-8" : "px-3"}`}>
      <div class="flex items-center gap-2">
        <span
          class={`h-2 w-2 shrink-0 rounded-full ${
            c.state === "running"
              ? "bg-emerald-500"
              : c.state === "paused"
                ? "bg-amber-500"
                : "bg-slate-300 dark:bg-slate-600"
          }`}
        ></span>
        <button
          type="button"
          onclick={() => openDetail(c.id, "logs")}
          class="truncate font-medium text-slate-900 transition hover:text-sky-600 hover:underline dark:text-white dark:hover:text-sky-400"
          title="Open container details"
        >
          {displayName}
        </button>
      </div>
      <div class="mt-0.5 flex items-center gap-2 pl-4 text-[11px] text-slate-400">
        <button
          type="button"
          onclick={() => copyContainerId(c.id)}
          title="Click to copy Container ID"
          class="font-mono hover:text-sky-600 dark:hover:text-sky-400"
        >
          {copiedId === c.id ? "Copied!" : c.id}
        </button>
        {#if !indented && c.compose_project}
          <span class="rounded bg-sky-50 px-1.5 py-0.5 text-[10px] font-medium text-sky-700 dark:bg-sky-500/15 dark:text-sky-400">
            {c.compose_project}
          </span>
        {/if}
      </div>
    </td>
    <td class="px-3 py-3 font-mono text-xs text-slate-700 dark:text-slate-300">
      {c.image}
    </td>
    <td class="px-3 py-3">
      <span class={`inline-flex rounded-full border px-2 py-0.5 text-[11px] font-medium ${stateBadgeClass(c.state)}`}>
        {c.state}
      </span>
      <div class="mt-0.5 text-[11px] text-slate-400">{c.status}</div>
    </td>
    <td class="px-3 py-3">
      {#if c.ports.length > 0}
        <div class="flex flex-wrap gap-1">
          {#each c.ports as port}
            {@const hostPort = extractHostPort(port)}
            {#if hostPort}
              <button
                type="button"
                onclick={() => openPort(port)}
                title={`Open http://localhost:${hostPort} in browser`}
                class="inline-flex items-center gap-1 rounded bg-sky-50 px-1.5 py-0.5 font-mono text-[11px] text-sky-700 transition hover:bg-sky-100 dark:bg-sky-500/15 dark:text-sky-300 dark:hover:bg-sky-500/25"
              >
                {port}
                <Icon name="external" class="h-2.5 w-2.5" />
              </button>
            {:else}
              <span class="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-[11px] text-slate-700 dark:bg-slate-800 dark:text-slate-300">
                {port}
              </span>
            {/if}
          {/each}
        </div>
      {:else}
        <span class="text-xs text-slate-400">—</span>
      {/if}
    </td>
    <td class="px-4 py-3 text-right">
      <div class="inline-flex items-center gap-0.5">
        {#if c.state === "running"}
          <button
            type="button"
            onclick={() => engineStore.runContainerAction(c.id, "stop")}
            disabled={engineStore.busy}
            title="Stop container"
            aria-label={`Stop ${displayName}`}
            class="rounded-lg p-1.5 text-amber-600 transition hover:bg-amber-50 disabled:opacity-40 dark:text-amber-400 dark:hover:bg-amber-500/15"
          >
            <Icon name="stop" class="h-4 w-4" />
          </button>
          <button
            type="button"
            onclick={() => engineStore.runContainerAction(c.id, "restart")}
            disabled={engineStore.busy}
            title="Restart container"
            aria-label={`Restart ${displayName}`}
            class="rounded-lg p-1.5 text-sky-600 transition hover:bg-sky-50 disabled:opacity-40 dark:text-sky-400 dark:hover:bg-sky-500/15"
          >
            <Icon name="refresh" class="h-4 w-4" />
          </button>
          <button
            type="button"
            onclick={() => openShell(c.id)}
            title="Open terminal shell (exec)"
            aria-label={`Open shell in ${displayName}`}
            class="rounded-lg p-1.5 text-emerald-600 transition hover:bg-emerald-50 dark:text-emerald-400 dark:hover:bg-emerald-500/15"
          >
            <Icon name="terminal" class="h-4 w-4" />
          </button>
        {:else}
          <button
            type="button"
            onclick={() => engineStore.runContainerAction(c.id, "start")}
            disabled={engineStore.busy}
            title="Start container"
            aria-label={`Start ${displayName}`}
            class="rounded-lg p-1.5 text-emerald-600 transition hover:bg-emerald-50 disabled:opacity-40 dark:text-emerald-400 dark:hover:bg-emerald-500/15"
          >
            <Icon name="play" class="h-4 w-4" />
          </button>
        {/if}
        <button
          type="button"
          onclick={() => openDetail(c.id, "logs")}
          title="View logs"
          aria-label={`View logs for ${displayName}`}
          class="rounded-lg p-1.5 text-slate-600 transition hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800"
        >
          <Icon name="logs" class="h-4 w-4" />
        </button>
        <button
          type="button"
          onclick={() => openDetail(c.id, "inspect")}
          title="Inspect & live stats"
          aria-label={`Inspect ${displayName}`}
          class="rounded-lg p-1.5 text-slate-600 transition hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800"
        >
          <Icon name="info" class="h-4 w-4" />
        </button>
        <button
          type="button"
          onclick={() => confirmDeleteSingle(c)}
          disabled={engineStore.busy}
          title="Delete container"
          aria-label={`Delete ${displayName}`}
          class="rounded-lg p-1.5 text-rose-600 transition hover:bg-rose-50 disabled:opacity-40 dark:text-rose-400 dark:hover:bg-rose-500/15"
        >
          <Icon name="trash" class="h-4 w-4" />
        </button>
      </div>
    </td>
  </tr>
{/snippet}

<RunContainerModal
  open={runModalOpen}
  allowImageEdit={true}
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
{/if}
