<script lang="ts">
  import { onMount } from "svelte";
  import {
    daemonApi,
    type ContainerActionKind,
    type ContainerSummary,
  } from "../../lib/api/daemon";
  import ConfirmModal from "../../lib/components/ConfirmModal.svelte";
  import EngineSleepingCard from "../../lib/components/EngineSleepingCard.svelte";
  import HelpTip from "../../lib/components/HelpTip.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import RefreshButton from "../../lib/components/RefreshButton.svelte";
  import RunContainerModal from "../../lib/components/RunContainerModal.svelte";
  import SearchInput from "../../lib/components/SearchInput.svelte";
  import Tooltip from "../../lib/components/Tooltip.svelte";
  import { i18n } from "../../lib/i18n/index.svelte";
  import { engineStore, type ContainerDetailTab } from "../../lib/stores/engine.svelte";
  import { useConfirm } from "../../lib/utils/confirm.svelte";
  import ContainerDetailView from "./ContainerDetailView.svelte";

  type StatusFilter = "all" | "running" | "stopped";
  type ContainerSortKey = "name" | "image" | "state";

  interface ContainerGroup {
    name: string;
    items: ContainerSummary[];
    runningCount: number;
  }

  let query = $state("");
  let statusFilter = $state<StatusFilter>("all");
  let sortKey = $state<ContainerSortKey>("name");
  let sortDir = $state<"asc" | "desc">("asc");
  let groupByStack = $state(true);
  let collapsedGroups = $state<Record<string, boolean>>({});
  let selectedIds = $state<Set<string>>(new Set());
  let copiedId = $state<string | null>(null);
  let openingShellId = $state<string | null>(null);

  function toggleSort(key: ContainerSortKey) {
    if (sortKey === key) {
      sortDir = sortDir === "asc" ? "desc" : "asc";
    } else {
      sortKey = key;
      sortDir = key === "state" ? "desc" : "asc";
    }
  }

  function compareContainers(a: ContainerSummary, b: ContainerSummary): number {
    const dir = sortDir === "asc" ? 1 : -1;
    if (sortKey === "image") return a.image.localeCompare(b.image) * dir;
    if (sortKey === "state") {
      const rank = (s: string) => (s === "running" ? 2 : s === "paused" ? 1 : 0);
      const diff = rank(a.state) - rank(b.state);
      if (diff !== 0) return diff * dir;
    }
    const aName = a.names[0] || a.id;
    const bName = b.names[0] || b.id;
    return aName.localeCompare(bName) * dir;
  }

  const confirmState = useConfirm();

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
        items: [...items].sort(compareContainers),
        runningCount: items.filter((c) => c.state === "running").length,
      }));

    return { groups, standalone: [...standalone].sort(compareContainers) };
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
        title: i18n.t.confirm.deleteStackTitle(group.name),
        message: i18n.t.confirm.deleteStackMsg(group.name, targetIds.length),
        label: i18n.t.confirm.deleteStackBtn,
        danger: true,
        action: async () => {
          await engineStore.runContainersBatch(targetIds, "remove", `group:${group.name}`);
          const next = new Set(selectedIds);
          for (const id of targetIds) next.delete(id);
          selectedIds = next;
        },
      });
      return;
    }

    await engineStore.runContainersBatch(targetIds, action, `group:${group.name}`);
  }

  let stackLogsOpen = $state(false);
  let stackLogsTitle = $state("");
  let stackLogsText = $state("");
  let stackLogsLoading = $state(false);

  async function openStackLogs(group: ContainerGroup) {
    stackLogsTitle = group.name;
    stackLogsOpen = true;
    stackLogsLoading = true;
    stackLogsText = "";
    try {
      const results = await Promise.all(
        group.items.map(async (c) => {
          const cname = c.names[0] || c.id;
          const res = await daemonApi.getContainerLogs(c.id, 80);
          if (res.status === "container_logs") {
            return `=== ${cname} ===\n${res.data.logs.trim()}`;
          }
          return `=== ${cname} ===\n(no logs)`;
        }),
      );
      stackLogsText = results.join("\n\n");
    } finally {
      stackLogsLoading = false;
    }
  }

  async function runSelectedAction(action: ContainerActionKind) {
    const targetIds = filterTargetIds(selectedContainers, action);
    if (targetIds.length === 0) return;

    if (action === "remove") {
      confirmState.request({
        title: i18n.t.confirm.deleteSelectedContainersTitle(targetIds.length),
        message: i18n.t.confirm.deleteSelectedContainersMsg(targetIds.length),
        label: i18n.t.confirm.deleteSelectedBtn,
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
      title: i18n.t.confirm.stopAllTitle(ids.length),
      message: i18n.t.confirm.stopAllMsg,
      label: i18n.t.confirm.stopAllBtn,
      danger: false,
      action: async () => {
        await engineStore.runContainersBatch(ids, "stop", "containers:stop-all");
      },
    });
  }

  function confirmDeleteSingle(c: ContainerSummary) {
    const displayName = c.names[0] || c.id;
    confirmState.request({
      title: i18n.t.confirm.deleteContainerTitle(displayName),
      message: i18n.t.confirm.deleteContainerMsg(displayName, c.id),
      label: i18n.t.common.delete,
      danger: true,
      action: async () => {
        await engineStore.runContainerAction(c.id, "remove");
        const next = new Set(selectedIds);
        next.delete(c.id);
        selectedIds = next;
      },
    });
  }

  function openDetail(id: string, tab: ContainerDetailTab = "logs") {
    engineStore.openContainerDetail(id, tab);
  }

  async function openShell(id: string) {
    if (openingShellId) return;
    openingShellId = id;
    try {
      await daemonApi.openContainerShell(id);
    } catch (e) {
      engineStore.error = String(e);
    } finally {
      openingShellId = null;
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

{#if engineStore.detailContainerId}
  <ContainerDetailView
    containerId={engineStore.detailContainerId}
    initialTab={engineStore.detailInitialTab}
    onBack={() => engineStore.closeContainerDetail()}
    onDeleted={() => engineStore.closeContainerDetail()}
  />
{:else}
<section class="space-y-4">
  <!-- Header -->
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div>
      <div class="flex items-center gap-2">
        <h2 class="text-lg font-semibold text-slate-900 dark:text-white">{i18n.t.containers.title}</h2>
        <HelpTip
          title={i18n.t.containers.helpTitle}
          align="left"
          position="bottom"
          text={i18n.t.containers.helpText}
        />
      </div>
      <p class="text-xs text-slate-500 dark:text-slate-400">
        {i18n.t.containers.summary(runningCount, stoppedCount, totalCount)}
      </p>
    </div>

    {#if engineStore.state !== "stopped"}
      <div class="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onclick={() => (runModalOpen = true)}
          disabled={engineStore.engineBusy}
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3 py-1.5 text-xs font-medium text-white transition hover:bg-sky-500 disabled:opacity-50"
        >
          <Icon name="plus" class="h-3.5 w-3.5" />
          {i18n.t.containers.runContainer}
        </button>

        {#if runningCount > 0}
          <button
            type="button"
            onclick={stopAllRunning}
            disabled={engineStore.engineBusy || engineStore.isPending("containers:stop-all:stop")}
            class="inline-flex items-center gap-1.5 rounded-lg border border-rose-200 bg-rose-50 px-3 py-1.5 text-xs font-medium text-rose-700 transition hover:bg-rose-100 disabled:opacity-50 dark:border-rose-500/30 dark:bg-rose-500/15 dark:text-rose-300 dark:hover:bg-rose-500/25"
            title={i18n.t.containers.stopAll(runningCount)}
          >
            <Icon
              name={engineStore.isPending("containers:stop-all:stop") ? "spinner" : "stop"}
              class={`h-3.5 w-3.5 ${engineStore.isPending("containers:stop-all:stop") ? "animate-spin" : ""}`}
            />
            {i18n.t.containers.stopAll(runningCount)}
          </button>
        {/if}

        <Tooltip text={i18n.t.containers.groupByStackTooltip} position="bottom">
          <button
            type="button"
            onclick={() => (groupByStack = !groupByStack)}
            class={`inline-flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-xs font-medium transition ${
              groupByStack
                ? "border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-500/30 dark:bg-sky-500/15 dark:text-sky-300"
                : "border-slate-200 bg-white text-slate-600 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300"
            }`}
          >
            <Icon name="images" class="h-3.5 w-3.5" />
            {i18n.t.containers.groupByStack}
          </button>
        </Tooltip>

        <RefreshButton
          onRefresh={() => engineStore.refreshContainers()}
          disabled={engineStore.loadingResources}
        />
      </div>
    {/if}
  </div>

  {#if engineStore.state === "stopped"}
    <EngineSleepingCard resourceLabel={i18n.t.containers.resourceLabel} />
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
          {i18n.t.common.all} ({totalCount})
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
          {i18n.t.containers.runningFilter(runningCount)}
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
          {i18n.t.containers.stoppedFilter(stoppedCount)}
        </button>
      </div>

      <div class="flex items-center gap-2">
        {#if groupByStack && grouped.groups.length > 0}
          <button
            type="button"
            onclick={() => setAllGroupsCollapsed(false)}
            class="rounded-md px-2 py-1 text-xs font-medium text-slate-500 hover:bg-slate-100 hover:text-slate-800 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
          >
            {i18n.t.containers.expandAll}
          </button>
          <button
            type="button"
            onclick={() => setAllGroupsCollapsed(true)}
            class="rounded-md px-2 py-1 text-xs font-medium text-slate-500 hover:bg-slate-100 hover:text-slate-800 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
          >
            {i18n.t.containers.collapseAll}
          </button>
        {/if}

        <SearchInput bind:value={query} placeholder={i18n.t.containers.searchPlaceholder} />
      </div>
    </div>

    <!-- Bulk Action Bar when items selected -->
    {#if selectedContainers.length > 0}
      <div class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-sky-200 bg-sky-50/90 px-4 py-2.5 dark:border-sky-500/30 dark:bg-sky-500/10">
        <div class="flex items-center gap-2 text-xs font-semibold text-sky-800 dark:text-sky-300">
          <span>{i18n.t.containers.selectedCount(selectedContainers.length)}</span>
        </div>
        <div class="flex flex-wrap items-center gap-1.5">
          <button
            type="button"
            onclick={() => runSelectedAction("start")}
            disabled={engineStore.busy || selectedContainers.every((c) => c.state === "running")}
            class="inline-flex items-center gap-1 rounded-md bg-emerald-600 px-2.5 py-1 text-xs font-medium text-white transition hover:bg-emerald-500 disabled:opacity-40"
          >
            <Icon
              name={engineStore.isPending("containers:batch:start") ? "spinner" : "play"}
              class={`h-3 w-3 ${engineStore.isPending("containers:batch:start") ? "animate-spin" : ""}`}
            />
            {i18n.t.common.start}
          </button>
          <button
            type="button"
            onclick={() => runSelectedAction("stop")}
            disabled={engineStore.busy || selectedContainers.every((c) => c.state !== "running")}
            class="inline-flex items-center gap-1 rounded-md bg-amber-600 px-2.5 py-1 text-xs font-medium text-white transition hover:bg-amber-500 disabled:opacity-40"
          >
            <Icon
              name={engineStore.isPending("containers:batch:stop") ? "spinner" : "stop"}
              class={`h-3 w-3 ${engineStore.isPending("containers:batch:stop") ? "animate-spin" : ""}`}
            />
            {i18n.t.common.stop}
          </button>
          <button
            type="button"
            onclick={() => runSelectedAction("restart")}
            disabled={engineStore.busy || selectedContainers.every((c) => c.state !== "running")}
            class="inline-flex items-center gap-1 rounded-md border border-slate-300 bg-white px-2.5 py-1 text-xs font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-40 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200"
          >
            {#if engineStore.isPending("containers:batch:restart")}
              <Icon name="spinner" class="h-3 w-3 animate-spin" />
            {/if}
            {i18n.t.common.restart}
          </button>
          <button
            type="button"
            onclick={() => runSelectedAction("remove")}
            disabled={engineStore.busy}
            class="inline-flex items-center gap-1 rounded-md bg-rose-600 px-2.5 py-1 text-xs font-medium text-white transition hover:bg-rose-500 disabled:opacity-40"
          >
            {#if engineStore.isPending("containers:batch:remove")}
              <Icon name="spinner" class="h-3 w-3 animate-spin" />
            {/if}
            {i18n.t.common.delete}
          </button>
          <button
            type="button"
            onclick={() => (selectedIds = new Set())}
            class="ml-1 rounded-md px-2 py-1 text-xs font-medium text-slate-600 hover:bg-white/70 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            {i18n.t.common.clear}
          </button>
        </div>
      </div>
    {/if}

    <!-- Container Table -->
    {#if filtered.length === 0}
      <div class="rounded-2xl border border-dashed border-slate-300 bg-white p-10 text-center text-sm text-slate-500 dark:border-slate-800 dark:bg-slate-900/30 dark:text-slate-400">
        {#if totalCount === 0}
          {i18n.t.containers.emptyHintPrefix} <strong>{i18n.t.containers.runContainer}</strong> {i18n.t.containers.emptyHintMiddle}
          <code class="rounded bg-slate-100 px-1.5 py-0.5 font-mono text-sky-700 dark:bg-slate-800 dark:text-sky-400">
            nanodock run -d -p 8080:80 nginx:alpine
          </code>
          {i18n.t.containers.emptyHintSuffix}
        {:else}
          <p>{i18n.t.containers.emptyFiltered}</p>
          <button
            type="button"
            onclick={() => {
              query = "";
              statusFilter = "all";
            }}
            class="mt-3 rounded-lg border border-slate-200 px-3 py-1 text-xs font-medium text-slate-700 transition hover:bg-slate-50 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            {i18n.t.nav.clearFilter}
          </button>
        {/if}
      </div>
    {:else}
      <div class="overflow-x-auto rounded-2xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900/50">
        <table class="w-full min-w-[740px] text-left text-sm">
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
              <th class="px-3 py-3">
                <button
                  type="button"
                  onclick={() => toggleSort("name")}
                  class="inline-flex items-center gap-1 uppercase tracking-wider hover:text-slate-800 dark:hover:text-slate-200"
                >
                  {i18n.t.containers.colName}
                  {#if sortKey === "name"}
                    <span class="text-sky-600 dark:text-sky-400">{sortDir === "asc" ? "↑" : "↓"}</span>
                  {/if}
                </button>
              </th>
              <th class="px-3 py-3">
                <button
                  type="button"
                  onclick={() => toggleSort("image")}
                  class="inline-flex items-center gap-1 uppercase tracking-wider hover:text-slate-800 dark:hover:text-slate-200"
                >
                  {i18n.t.containers.colImage}
                  {#if sortKey === "image"}
                    <span class="text-sky-600 dark:text-sky-400">{sortDir === "asc" ? "↑" : "↓"}</span>
                  {/if}
                </button>
              </th>
              <th class="px-3 py-3">
                <button
                  type="button"
                  onclick={() => toggleSort("state")}
                  class="inline-flex items-center gap-1 uppercase tracking-wider hover:text-slate-800 dark:hover:text-slate-200"
                >
                  {i18n.t.containers.colStatus}
                  {#if sortKey === "state"}
                    <span class="text-sky-600 dark:text-sky-400">{sortDir === "asc" ? "↑" : "↓"}</span>
                  {/if}
                </button>
              </th>
              <th class="px-3 py-3">{i18n.t.containers.colPorts}</th>
              <th class="px-4 py-3 text-right">{i18n.t.containers.colActions}</th>
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
                      {i18n.t.containers.stackRunning(group.runningCount, group.items.length)}
                    </span>
                  </button>
                </td>
                <td class="px-4 py-2.5 text-right">
                  <div class="inline-flex items-center gap-0.5">
                    {#if group.runningCount < group.items.length}
                      <Tooltip text={i18n.t.common.start}>
                        <button
                          type="button"
                          onclick={() => runGroupAction(group, "start")}
                          disabled={engineStore.engineBusy || engineStore.isPrefixPending(`group:${group.name}:`)}
                          aria-label={`Start all in stack ${group.name}`}
                          class="rounded-lg p-1.5 text-emerald-600 transition hover:bg-emerald-50 disabled:opacity-40 dark:text-emerald-400 dark:hover:bg-emerald-500/15"
                        >
                          <Icon
                            name={engineStore.isPending(`group:${group.name}:start`) ? "spinner" : "play"}
                            class={`h-4 w-4 ${engineStore.isPending(`group:${group.name}:start`) ? "animate-spin" : ""}`}
                          />
                        </button>
                      </Tooltip>
                    {/if}
                    {#if group.runningCount > 0}
                      <Tooltip text={i18n.t.common.stop}>
                        <button
                          type="button"
                          onclick={() => runGroupAction(group, "stop")}
                          disabled={engineStore.engineBusy || engineStore.isPrefixPending(`group:${group.name}:`)}
                          aria-label={`Stop all in stack ${group.name}`}
                          class="rounded-lg p-1.5 text-amber-600 transition hover:bg-amber-50 disabled:opacity-40 dark:text-amber-400 dark:hover:bg-amber-500/15"
                        >
                          <Icon
                            name={engineStore.isPending(`group:${group.name}:stop`) ? "spinner" : "stop"}
                            class={`h-4 w-4 ${engineStore.isPending(`group:${group.name}:stop`) ? "animate-spin" : ""}`}
                          />
                        </button>
                      </Tooltip>
                      <Tooltip text={i18n.t.common.restart}>
                        <button
                          type="button"
                          onclick={() => runGroupAction(group, "restart")}
                          disabled={engineStore.engineBusy || engineStore.isPrefixPending(`group:${group.name}:`)}
                          aria-label={`Restart running in stack ${group.name}`}
                          class="rounded-lg p-1.5 text-sky-600 transition hover:bg-sky-50 disabled:opacity-40 dark:text-sky-400 dark:hover:bg-sky-500/15"
                        >
                          <Icon
                            name="refresh"
                            class={`h-4 w-4 ${engineStore.isPending(`group:${group.name}:restart`) ? "animate-spin" : ""}`}
                          />
                        </button>
                      </Tooltip>
                    {/if}
                    <Tooltip text={i18n.t.extra.composeLogs}>
                      <button
                        type="button"
                        onclick={() => openStackLogs(group)}
                        aria-label={`${i18n.t.extra.composeLogs}: ${group.name}`}
                        class="rounded-lg p-1.5 text-slate-600 transition hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800"
                      >
                        <Icon name="logs" class="h-4 w-4" />
                      </button>
                    </Tooltip>
                    <Tooltip text={i18n.t.extra.composeDown} align="right">
                      <button
                        type="button"
                        onclick={() => runGroupAction(group, "remove")}
                        disabled={engineStore.engineBusy || engineStore.isPrefixPending(`group:${group.name}:`)}
                        aria-label={`Delete stack ${group.name}`}
                        class="rounded-lg p-1.5 text-rose-600 transition hover:bg-rose-50 disabled:opacity-40 dark:text-rose-400 dark:hover:bg-rose-500/15"
                      >
                        <Icon
                          name={engineStore.isPending(`group:${group.name}:remove`) ? "spinner" : "trash"}
                          class={`h-4 w-4 ${engineStore.isPending(`group:${group.name}:remove`) ? "animate-spin" : ""}`}
                        />
                      </button>
                    </Tooltip>
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
  {@const rowBusy = engineStore.engineBusy || engineStore.isPrefixPending(`container:${c.id}:`)}
  <tr
    onclick={(e) => {
      if (!(e.target as HTMLElement).closest("button, input, a")) {
        openDetail(c.id, "logs");
      }
    }}
    class="cursor-pointer transition hover:bg-slate-50/80 dark:hover:bg-slate-800/40"
  >
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
          {copiedId === c.id ? i18n.t.common.copied : c.id}
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
          <Tooltip text={i18n.t.containers.stopTooltip}>
            <button
              type="button"
              onclick={() => engineStore.runContainerAction(c.id, "stop")}
              disabled={rowBusy}
              aria-label={`Stop ${displayName}`}
              class="rounded-lg p-1.5 text-amber-600 transition hover:bg-amber-50 disabled:opacity-40 dark:text-amber-400 dark:hover:bg-amber-500/15"
            >
              <Icon
                name={engineStore.isPending(`container:${c.id}:stop`) ? "spinner" : "stop"}
                class={`h-4 w-4 ${engineStore.isPending(`container:${c.id}:stop`) ? "animate-spin" : ""}`}
              />
            </button>
          </Tooltip>
          <Tooltip text={i18n.t.containers.restartTooltip}>
            <button
              type="button"
              onclick={() => engineStore.runContainerAction(c.id, "restart")}
              disabled={rowBusy}
              aria-label={`Restart ${displayName}`}
              class="rounded-lg p-1.5 text-sky-600 transition hover:bg-sky-50 disabled:opacity-40 dark:text-sky-400 dark:hover:bg-sky-500/15"
            >
              <Icon
                name="refresh"
                class={`h-4 w-4 ${engineStore.isPending(`container:${c.id}:restart`) ? "animate-spin" : ""}`}
              />
            </button>
          </Tooltip>
          <Tooltip text={i18n.t.containers.shellTooltip}>
            <button
              type="button"
              onclick={() => openShell(c.id)}
              disabled={openingShellId === c.id}
              aria-label={`Open shell in ${displayName}`}
              class="rounded-lg p-1.5 text-emerald-600 transition hover:bg-emerald-50 disabled:opacity-40 dark:text-emerald-400 dark:hover:bg-emerald-500/15"
            >
              <Icon
                name={openingShellId === c.id ? "spinner" : "terminal"}
                class={`h-4 w-4 ${openingShellId === c.id ? "animate-spin" : ""}`}
              />
            </button>
          </Tooltip>
        {:else}
          <Tooltip text={i18n.t.containers.startTooltip}>
            <button
              type="button"
              onclick={() => engineStore.runContainerAction(c.id, "start")}
              disabled={rowBusy}
              aria-label={`Start ${displayName}`}
              class="rounded-lg p-1.5 text-emerald-600 transition hover:bg-emerald-50 disabled:opacity-40 dark:text-emerald-400 dark:hover:bg-emerald-500/15"
            >
              <Icon
                name={engineStore.isPending(`container:${c.id}:start`) ? "spinner" : "play"}
                class={`h-4 w-4 ${engineStore.isPending(`container:${c.id}:start`) ? "animate-spin" : ""}`}
              />
            </button>
          </Tooltip>
        {/if}
        <Tooltip text={i18n.t.containers.logsTooltip}>
          <button
            type="button"
            onclick={() => openDetail(c.id, "logs")}
            aria-label={`View logs for ${displayName}`}
            class="rounded-lg p-1.5 text-slate-600 transition hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            <Icon name="logs" class="h-4 w-4" />
          </button>
        </Tooltip>
        <Tooltip text={i18n.t.containers.inspectTooltip}>
          <button
            type="button"
            onclick={() => openDetail(c.id, "inspect")}
            aria-label={`Inspect ${displayName}`}
            class="rounded-lg p-1.5 text-slate-600 transition hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            <Icon name="info" class="h-4 w-4" />
          </button>
        </Tooltip>
        <Tooltip text={i18n.t.containers.deleteTooltip} align="right">
          <button
            type="button"
            onclick={() => confirmDeleteSingle(c)}
            disabled={rowBusy}
            aria-label={`Delete ${displayName}`}
            class="rounded-lg p-1.5 text-rose-600 transition hover:bg-rose-50 disabled:opacity-40 dark:text-rose-400 dark:hover:bg-rose-500/15"
          >
            <Icon
              name={engineStore.isPending(`container:${c.id}:remove`) ? "spinner" : "trash"}
              class={`h-4 w-4 ${engineStore.isPending(`container:${c.id}:remove`) ? "animate-spin" : ""}`}
            />
          </button>
        </Tooltip>
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

{#if stackLogsOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
  <div
    onclick={(e) => {
      if (e.target === e.currentTarget) stackLogsOpen = false;
    }}
    onkeydown={(e) => {
      if (e.key === "Escape") stackLogsOpen = false;
    }}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    aria-label={`${stackLogsTitle} — ${i18n.t.extra.composeLogs}`}
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/60 p-4 backdrop-blur-xs"
  >
    <div class="flex max-h-[85vh] w-full max-w-4xl flex-col rounded-2xl border border-slate-200 bg-white shadow-2xl dark:border-slate-800 dark:bg-slate-900">
      <div class="flex items-center justify-between border-b border-slate-200 px-5 py-3.5 dark:border-slate-800">
        <h3 class="text-sm font-semibold text-slate-900 dark:text-slate-100">{stackLogsTitle} — {i18n.t.extra.composeLogs}</h3>
        <button
          type="button"
          onclick={() => (stackLogsOpen = false)}
          aria-label={i18n.t.common.cancel}
          class="rounded-lg p-1.5 text-slate-400 hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-slate-800 dark:hover:text-slate-100"
        >
          <Icon name="close" class="h-4 w-4" />
        </button>
      </div>
      <div class="flex-1 overflow-y-auto p-5">
        {#if stackLogsLoading}
          <p class="text-xs text-slate-500 dark:text-slate-400">{i18n.t.common.loading}</p>
        {:else}
          <pre class="max-h-[65vh] overflow-y-auto rounded-lg border border-slate-200 bg-slate-950 p-3.5 whitespace-pre-wrap break-all font-mono text-xs text-slate-200 dark:border-slate-800">{stackLogsText || "(empty)"}</pre>
        {/if}
      </div>
    </div>
  </div>
{/if}
{/if}
