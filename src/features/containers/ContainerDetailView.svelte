<script lang="ts">
  import { onMount } from "svelte";
  import {
    daemonApi,
    type ContainerInspect,
    type ContainerStats,
  } from "../../lib/api/daemon";
  import ConfirmModal from "../../lib/components/ConfirmModal.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import RefreshButton from "../../lib/components/RefreshButton.svelte";
  import SearchInput from "../../lib/components/SearchInput.svelte";
  import { i18n } from "../../lib/i18n/index.svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";
  import { useConfirm } from "../../lib/utils/confirm.svelte";
  import { formatBytes, formatIsoDate } from "../../lib/utils/format";
  import ContainerFilesTab from "./ContainerFilesTab.svelte";
  import ContainerTerminalTab from "./ContainerTerminalTab.svelte";

  export type DetailTab = "logs" | "stats" | "inspect" | "env" | "mounts" | "terminal" | "files";

  let {
    containerId,
    initialTab = "logs",
    onBack,
    onDeleted,
  }: {
    containerId: string;
    initialTab?: DetailTab;
    onBack: () => void;
    onDeleted: () => void;
  } = $props();

  let activeTab = $state<DetailTab>("logs");
  const confirmState = useConfirm();

  // Inspect state
  let inspectLoading = $state(false);
  let inspectError = $state<string | null>(null);
  let info = $state<ContainerInspect | null>(null);

  // Logs state
  let logsContent = $state("");
  let logsTail = $state(200);
  let logsQuery = $state("");
  let loadingLogs = $state(false);
  let liveLogs = $state(true);
  let copiedLogs = $state(false);
  let logsPre = $state<HTMLPreElement | null>(null);

  // Stats state (with lightweight rolling history for sparkline)
  let stats = $state<ContainerStats | null>(null);
  let loadingStats = $state(false);
  let cpuHistory = $state<number[]>([]);
  let memHistory = $state<number[]>([]);

  // Env & Mounts search/copy
  let envQuery = $state("");
  let copiedKey = $state<string | null>(null);
  let openingShell = $state(false);

  const summary = $derived(
    engineStore.containers.find((c) => c.id === containerId) ?? null,
  );
  const displayName = $derived(
    summary?.names[0] || info?.name || containerId,
  );
  const currentState = $derived(summary?.state ?? info?.state ?? "unknown");
  const isRunning = $derived(currentState === "running");
  const ports = $derived(summary?.ports ?? info?.ports ?? []);

  const filteredLogsText = $derived.by(() => {
    if (!logsQuery.trim()) return logsContent;
    const q = logsQuery.trim().toLowerCase();
    return logsContent
      .split("\n")
      .filter((line) => line.toLowerCase().includes(q))
      .join("\n");
  });

  const filteredEnv = $derived.by(() => {
    if (!info) return [];
    const q = envQuery.trim().toLowerCase();
    if (!q) return info.env;
    return info.env.filter((item) => item.toLowerCase().includes(q));
  });

  const memPercent = $derived.by(() => {
    if (!stats || stats.mem_limit_bytes <= 0) return 0;
    return Math.min(100, (stats.mem_usage_bytes / stats.mem_limit_bytes) * 100);
  });

  onMount(() => {
    activeTab = initialTab;
    void loadInspect();
    void fetchLogs();
  });

  // Poll logs only when on Logs tab, Live is ON, container is running, and document is visible
  $effect(() => {
    if (activeTab !== "logs" || !liveLogs || !isRunning) return;
    const timer = setInterval(() => {
      if (document.hidden || loadingLogs) return;
      void fetchLogs(true);
    }, 2000);
    return () => clearInterval(timer);
  });

  // Poll stats only when on Stats tab, container is running, and document is visible
  $effect(() => {
    if (activeTab !== "stats" || !isRunning) return;
    void fetchStats();
    const timer = setInterval(() => {
      if (document.hidden || loadingStats) return;
      void fetchStats();
    }, 2000);
    return () => clearInterval(timer);
  });

  async function loadInspect() {
    inspectLoading = true;
    inspectError = null;
    try {
      const resp = await daemonApi.inspectContainer(containerId);
      if (resp.status === "container_inspect") {
        info = resp.data;
      } else if (resp.status === "error") {
        inspectError = resp.data.message;
      }
    } catch (e) {
      inspectError = String(e);
    } finally {
      inspectLoading = false;
    }
  }

  async function fetchLogs(silent = false) {
    if (!silent) loadingLogs = true;
    try {
      const resp = await daemonApi.getContainerLogs(containerId, logsTail);
      if (resp.status === "container_logs") {
        logsContent = resp.data.logs || "(No logs output)";
        if (liveLogs && logsPre) {
          queueMicrotask(() => {
            if (logsPre) logsPre.scrollTop = logsPre.scrollHeight;
          });
        }
      } else if (resp.status === "error") {
        logsContent = `Error: ${resp.data.message}`;
      }
    } catch (e) {
      logsContent = `Error: ${String(e)}`;
    } finally {
      if (!silent) loadingLogs = false;
    }
  }

  async function fetchStats() {
    loadingStats = true;
    try {
      const resp = await daemonApi.getContainerStats(containerId);
      if (resp.status === "container_stats") {
        stats = resp.data;
        cpuHistory = [...cpuHistory.slice(-23), resp.data.cpu_percent];
        const memPct =
          resp.data.mem_limit_bytes > 0
            ? (resp.data.mem_usage_bytes / resp.data.mem_limit_bytes) * 100
            : 0;
        memHistory = [...memHistory.slice(-23), memPct];
      }
    } catch {
      // non-fatal
    } finally {
      loadingStats = false;
    }
  }

  async function handleAction(action: "start" | "stop" | "restart") {
    await engineStore.runContainerAction(containerId, action);
    await loadInspect();
    if (action === "restart" || action === "start") {
      void fetchLogs(true);
    }
  }

  function confirmDelete() {
    confirmState.request({
      title: `Delete container "${displayName}"?`,
      message: `This will force-remove "${displayName}" (${containerId}).`,
      label: "Delete",
      danger: true,
      action: async () => {
        await engineStore.runContainerAction(containerId, "remove");
        onDeleted();
      },
    });
  }

  async function openShell() {
    if (openingShell) return;
    openingShell = true;
    try {
      await daemonApi.openContainerShell(containerId);
    } catch (e) {
      engineStore.error = String(e);
    } finally {
      openingShell = false;
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

  async function copyText(key: string, text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedKey = key;
      setTimeout(() => {
        if (copiedKey === key) copiedKey = null;
      }, 1400);
    } catch {
      // ignore
    }
  }

  async function copyLogs() {
    try {
      await navigator.clipboard.writeText(filteredLogsText);
      copiedLogs = true;
      setTimeout(() => (copiedLogs = false), 1500);
    } catch {
      // ignore
    }
  }

  function sparklinePoints(values: number[], maxVal = 100): string {
    if (values.length === 0) return "";
    if (values.length === 1) {
      const y = 40 - Math.min(1, values[0] / maxVal) * 36;
      return `0,${y} 200,${y}`;
    }
    return values
      .map((v, i) => {
        const x = (i / (values.length - 1)) * 200;
        const y = 40 - Math.min(1, Math.max(0, v) / maxVal) * 36;
        return `${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(" ");
  }
</script>

<section class="flex h-full flex-col gap-4">
  <!-- Top Bar & Breadcrumb -->
  <div class="flex flex-wrap items-center justify-between gap-3 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
    <div class="min-w-0 space-y-2">
      <div class="flex items-center gap-2 text-xs text-slate-500 dark:text-slate-400">
        <button
          type="button"
          onclick={onBack}
          class="inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 font-medium text-slate-600 transition hover:bg-slate-100 hover:text-slate-900 dark:text-slate-300 dark:hover:bg-slate-800 dark:hover:text-white"
        >
          <Icon name="arrow-left" class="h-3.5 w-3.5" />
          Containers
        </button>
        <span>/</span>
        <span class="truncate font-mono text-slate-700 dark:text-slate-200">{displayName}</span>
      </div>

      <div class="flex flex-wrap items-center gap-2.5">
        <span
          class={`h-2.5 w-2.5 shrink-0 rounded-full ${
            isRunning
              ? "bg-emerald-500"
              : currentState === "paused"
                ? "bg-amber-500"
                : "bg-slate-300 dark:bg-slate-600"
          }`}
        ></span>
        <h2 class="text-lg font-bold text-slate-900 dark:text-white">{displayName}</h2>
        <span
          class={`rounded-full border px-2 py-0.5 text-[11px] font-medium ${
            isRunning
              ? "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-500/30 dark:bg-emerald-500/15 dark:text-emerald-400"
              : "border-slate-200 bg-slate-100 text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-400"
          }`}
        >
          {currentState}
        </span>
        <span class="rounded-md bg-slate-100 px-2 py-0.5 font-mono text-xs text-slate-600 dark:bg-slate-800 dark:text-slate-300">
          {summary?.image ?? info?.image ?? "—"}
        </span>
        <span class="font-mono text-xs text-slate-400">{containerId}</span>
      </div>

      {#if ports.length > 0}
        <div class="flex flex-wrap items-center gap-1.5 pt-0.5">
          {#each ports as port}
            {@const hostPort = extractHostPort(port)}
            {#if hostPort}
              <button
                type="button"
                onclick={() => openPort(port)}
                title={`Open http://localhost:${hostPort} in browser`}
                class="inline-flex items-center gap-1 rounded bg-sky-50 px-2 py-0.5 font-mono text-[11px] text-sky-700 transition hover:bg-sky-100 dark:bg-sky-500/15 dark:text-sky-300 dark:hover:bg-sky-500/25"
              >
                {port}
                <Icon name="external" class="h-2.5 w-2.5" />
              </button>
            {:else}
              <span class="rounded bg-slate-100 px-2 py-0.5 font-mono text-[11px] text-slate-600 dark:bg-slate-800 dark:text-slate-300">
                {port}
              </span>
            {/if}
          {/each}
        </div>
      {/if}
    </div>

    <div class="flex flex-wrap items-center gap-2">
      {#if isRunning}
        <button
          type="button"
          onclick={openShell}
          disabled={openingShell}
          class="inline-flex items-center gap-1.5 rounded-lg border border-emerald-200 bg-emerald-50 px-3 py-1.5 text-xs font-medium text-emerald-700 transition hover:bg-emerald-100 disabled:opacity-50 dark:border-emerald-500/30 dark:bg-emerald-500/15 dark:text-emerald-300"
        >
          <Icon
            name={openingShell ? "spinner" : "terminal"}
            class={`h-3.5 w-3.5 ${openingShell ? "animate-spin" : ""}`}
          />
          Open Terminal
        </button>
        <button
          type="button"
          onclick={() => handleAction("stop")}
          disabled={engineStore.busy}
          class="inline-flex items-center gap-1.5 rounded-lg bg-amber-600 px-3 py-1.5 text-xs font-medium text-white transition hover:bg-amber-500 disabled:opacity-50"
        >
          <Icon
            name={engineStore.isPending(`container:${containerId}:stop`) ? "spinner" : "stop"}
            class={`h-3.5 w-3.5 ${engineStore.isPending(`container:${containerId}:stop`) ? "animate-spin" : ""}`}
          />
          {i18n.t.common.stop}
        </button>
        <button
          type="button"
          onclick={() => handleAction("restart")}
          disabled={engineStore.busy}
          class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-3 py-1.5 text-xs font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200"
        >
          <Icon
            name="refresh"
            class={`h-3.5 w-3.5 ${engineStore.isPending(`container:${containerId}:restart`) ? "animate-spin" : ""}`}
          />
          {i18n.t.common.restart}
        </button>
      {:else}
        <button
          type="button"
          onclick={() => handleAction("start")}
          disabled={engineStore.busy}
          class="inline-flex items-center gap-1.5 rounded-lg bg-emerald-600 px-3 py-1.5 text-xs font-medium text-white transition hover:bg-emerald-500 disabled:opacity-50"
        >
          <Icon
            name={engineStore.isPending(`container:${containerId}:start`) ? "spinner" : "play"}
            class={`h-3.5 w-3.5 ${engineStore.isPending(`container:${containerId}:start`) ? "animate-spin" : ""}`}
          />
          {i18n.t.common.start}
        </button>
      {/if}
      <button
        type="button"
        onclick={confirmDelete}
        disabled={engineStore.busy}
        class="inline-flex items-center gap-1.5 rounded-lg border border-rose-200 bg-rose-50 px-3 py-1.5 text-xs font-medium text-rose-700 transition hover:bg-rose-100 disabled:opacity-50 dark:border-rose-500/30 dark:bg-rose-500/15 dark:text-rose-300"
      >
        <Icon
          name={engineStore.isPending(`container:${containerId}:remove`) ? "spinner" : "trash"}
          class={`h-3.5 w-3.5 ${engineStore.isPending(`container:${containerId}:remove`) ? "animate-spin" : ""}`}
        />
        {i18n.t.common.delete}
      </button>
    </div>
  </div>

  <!-- Sub-navigation Tabs -->
  <div class="flex flex-wrap items-center gap-1 border-b border-slate-200 pb-2 dark:border-slate-800">
    {#each [
      { id: "logs", label: "Logs", icon: "logs" },
      { id: "stats", label: "Stats", icon: "activity" },
      { id: "inspect", label: "Inspect", icon: "info" },
      { id: "env", label: `Environment (${info?.env.length ?? 0})`, icon: "settings" },
      { id: "mounts", label: `Mounts (${info?.mounts.length ?? 0})`, icon: "volumes" },
      { id: "terminal", label: i18n.t.detail.tabTerminal, icon: "terminal" },
      { id: "files", label: i18n.t.detail.tabFiles, icon: "folder" },
    ] as const as tab}
      <button
        type="button"
        onclick={() => (activeTab = tab.id)}
        class={`inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition ${
          activeTab === tab.id
            ? "bg-sky-50 text-sky-700 dark:bg-sky-500/15 dark:text-sky-300"
            : "text-slate-600 hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
        }`}
      >
        <Icon name={tab.icon} class="h-3.5 w-3.5" />
        {tab.label}
      </button>
    {/each}
  </div>

  <!-- Tab Content -->
  {#if activeTab === "logs"}
    <div class="flex min-h-[420px] flex-1 flex-col overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-900/50">
      <div class="flex flex-wrap items-center justify-between gap-2 border-b border-slate-200 px-4 py-2.5 dark:border-slate-800">
        <div class="flex flex-wrap items-center gap-2">
          <SearchInput bind:value={logsQuery} placeholder="Filter log lines..." class="w-52" />
          <select
            bind:value={logsTail}
            onchange={() => fetchLogs()}
            aria-label="Number of log lines"
            class="rounded-lg border border-slate-200 bg-white px-2.5 py-1.5 text-xs text-slate-700 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200"
          >
            <option value={100}>Last 100</option>
            <option value={200}>Last 200</option>
            <option value={500}>Last 500</option>
            <option value={1000}>Last 1000</option>
          </select>
        </div>

        <div class="flex items-center gap-2">
          <button
            type="button"
            onclick={() => (liveLogs = !liveLogs)}
            disabled={!isRunning}
            class={`inline-flex items-center gap-1.5 rounded-lg border px-2.5 py-1 text-xs font-medium transition disabled:opacity-40 ${
              liveLogs && isRunning
                ? "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-500/30 dark:bg-emerald-500/15 dark:text-emerald-300"
                : "border-slate-200 text-slate-600 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
            }`}
          >
            <span class={`h-1.5 w-1.5 rounded-full ${liveLogs && isRunning ? "animate-pulse bg-emerald-500" : "bg-slate-400"}`}></span>
            Live
          </button>
          <RefreshButton
            size="xs"
            onRefresh={() => fetchLogs()}
            disabled={loadingLogs}
            showNotice={false}
          />
          <button
            type="button"
            onclick={copyLogs}
            class="inline-flex items-center gap-1 rounded-lg border border-slate-200 px-2.5 py-1 text-xs font-medium text-slate-700 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            <Icon name={copiedLogs ? "check" : "copy"} class="h-3.5 w-3.5" />
            {copiedLogs ? "Copied!" : "Copy"}
          </button>
        </div>
      </div>

      <pre
        bind:this={logsPre}
        class="flex-1 overflow-auto bg-slate-950 p-4 font-mono text-xs leading-relaxed text-slate-200"
      >{loadingLogs ? "Loading logs..." : filteredLogsText || "(No matching log lines)"}</pre>
    </div>
  {:else if activeTab === "stats"}
    {#if !isRunning}
      <div class="rounded-2xl border border-dashed border-slate-300 bg-white p-10 text-center text-sm text-slate-500 dark:border-slate-800 dark:bg-slate-900/40 dark:text-slate-400">
        Container is not running. Start the container to monitor live CPU and memory usage.
      </div>
    {:else}
      <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
        <div class="rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/50">
          <div class="flex items-center justify-between">
            <span class="text-xs font-medium uppercase tracking-wider text-slate-400">CPU Usage</span>
            <span class="text-xs text-slate-400">Polling every 2s</span>
          </div>
          <p class="mt-2 text-3xl font-bold text-slate-900 dark:text-white">
            {stats ? `${stats.cpu_percent.toFixed(2)}%` : loadingStats ? "..." : "—"}
          </p>
          <div class="mt-3 h-2 w-full overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800">
            <div
              class="h-full rounded-full bg-sky-500 transition-all duration-300"
              style={`width: ${Math.min(100, stats?.cpu_percent ?? 0)}%`}
            ></div>
          </div>
          {#if cpuHistory.length > 1}
            <svg viewBox="0 0 200 44" class="mt-4 h-12 w-full text-sky-500" preserveAspectRatio="none">
              <polyline
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                points={sparklinePoints(cpuHistory, Math.max(100, ...cpuHistory))}
              />
            </svg>
          {/if}
        </div>

        <div class="rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/50">
          <div class="flex items-center justify-between">
            <span class="text-xs font-medium uppercase tracking-wider text-slate-400">Memory Usage</span>
            <span class="font-mono text-xs text-slate-400">
              {stats && stats.mem_limit_bytes > 0 ? `Limit: ${formatBytes(stats.mem_limit_bytes)}` : ""}
            </span>
          </div>
          <p class="mt-2 text-3xl font-bold text-slate-900 dark:text-white">
            {stats ? formatBytes(stats.mem_usage_bytes) : loadingStats ? "..." : "—"}
            {#if stats && stats.mem_limit_bytes > 0}
              <span class="ml-1.5 text-sm font-medium text-slate-400">
                ({memPercent.toFixed(1)}%)
              </span>
            {/if}
          </p>
          <div class="mt-3 h-2 w-full overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800">
            <div
              class="h-full rounded-full bg-emerald-500 transition-all duration-300"
              style={`width: ${memPercent}%`}
            ></div>
          </div>
          {#if memHistory.length > 1}
            <svg viewBox="0 0 200 44" class="mt-4 h-12 w-full text-emerald-500" preserveAspectRatio="none">
              <polyline
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                points={sparklinePoints(memHistory, 100)}
              />
            </svg>
          {/if}
        </div>
      </div>
    {/if}
  {:else if inspectLoading && !info}
    <p class="text-sm text-slate-500 dark:text-slate-400">Loading container details...</p>
  {:else if inspectError}
    <p class="text-sm text-rose-600 dark:text-rose-400">{inspectError}</p>
  {:else if info}
    {#if activeTab === "inspect"}
      <dl class="grid grid-cols-1 gap-3 sm:grid-cols-2">
        <div class="rounded-2xl border border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900/50">
          <dt class="text-[11px] font-medium uppercase text-slate-400">Image</dt>
          <dd class="mt-1 font-mono text-xs text-slate-800 dark:text-slate-200">{info.image}</dd>
        </div>
        <div class="rounded-2xl border border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900/50">
          <dt class="text-[11px] font-medium uppercase text-slate-400">State & Status</dt>
          <dd class="mt-1 text-xs text-slate-800 dark:text-slate-200">{info.state} ({info.status})</dd>
        </div>
        <div class="rounded-2xl border border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900/50">
          <dt class="text-[11px] font-medium uppercase text-slate-400">IP Address</dt>
          <dd class="mt-1 font-mono text-xs text-slate-800 dark:text-slate-200">{info.ip_address || "—"}</dd>
        </div>
        <div class="rounded-2xl border border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900/50">
          <dt class="text-[11px] font-medium uppercase text-slate-400">Networks</dt>
          <dd class="mt-1 font-mono text-xs text-slate-800 dark:text-slate-200">
            {info.networks.length > 0 ? info.networks.join(", ") : "—"}
          </dd>
        </div>
        <div class="rounded-2xl border border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900/50">
          <dt class="text-[11px] font-medium uppercase text-slate-400">Restart Policy</dt>
          <dd class="mt-1 font-mono text-xs text-slate-800 dark:text-slate-200">{info.restart_policy}</dd>
        </div>
        <div class="rounded-2xl border border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900/50">
          <dt class="text-[11px] font-medium uppercase text-slate-400">Created</dt>
          <dd class="mt-1 text-xs text-slate-800 dark:text-slate-200">{formatIsoDate(info.created)}</dd>
        </div>
        <div class="rounded-2xl border border-slate-200 bg-white p-4 sm:col-span-2 dark:border-slate-800 dark:bg-slate-900/50">
          <dt class="text-[11px] font-medium uppercase text-slate-400">Entrypoint / Cmd</dt>
          <dd class="mt-1 break-all font-mono text-xs text-slate-800 dark:text-slate-200">
            {[...info.entrypoint, ...info.cmd].join(" ") || "—"}
          </dd>
        </div>
      </dl>
    {:else if activeTab === "env"}
      <div class="space-y-3">
        <div class="flex items-center justify-between gap-3">
          <SearchInput bind:value={envQuery} placeholder="Filter environment variables..." />
          <span class="text-xs text-slate-400">{filteredEnv.length} variables</span>
        </div>

        {#if filteredEnv.length === 0}
          <p class="rounded-2xl border border-dashed border-slate-300 bg-white p-8 text-center text-xs text-slate-500 dark:border-slate-800 dark:bg-slate-900/40 dark:text-slate-400">
            No matching environment variables.
          </p>
        {:else}
          <div class="divide-y divide-slate-200 overflow-hidden rounded-2xl border border-slate-200 bg-white dark:divide-slate-800 dark:border-slate-800 dark:bg-slate-900/50">
            {#each filteredEnv as item}
              {@const eqIdx = item.indexOf("=")}
              {@const key = eqIdx >= 0 ? item.slice(0, eqIdx) : item}
              {@const val = eqIdx >= 0 ? item.slice(eqIdx + 1) : ""}
              <div class="flex items-center justify-between gap-3 px-4 py-2.5 font-mono text-xs">
                <div class="flex min-w-0 flex-1 gap-3">
                  <span class="w-52 shrink-0 truncate font-semibold text-sky-700 dark:text-sky-400" title={key}>
                    {key}
                  </span>
                  <span class="break-all text-slate-700 dark:text-slate-300">{val}</span>
                </div>
                <button
                  type="button"
                  onclick={() => copyText(`env:${key}`, val)}
                  title="Copy value"
                  aria-label={`Copy value of ${key}`}
                  class="shrink-0 rounded p-1 text-slate-400 transition hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-slate-800 dark:hover:text-slate-200"
                >
                  <Icon name={copiedKey === `env:${key}` ? "check" : "copy"} class="h-3.5 w-3.5" />
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {:else if activeTab === "mounts"}
      {#if info.mounts.length === 0}
        <p class="rounded-2xl border border-dashed border-slate-300 bg-white p-8 text-center text-xs text-slate-500 dark:border-slate-800 dark:bg-slate-900/40 dark:text-slate-400">
          No mounts configured.
        </p>
      {:else}
        <div class="space-y-2.5">
          {#each info.mounts as m}
            <div class="rounded-2xl border border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900/50">
              <div class="flex items-center justify-between">
                <span class="rounded bg-slate-100 px-2 py-0.5 text-[10px] font-semibold uppercase text-slate-700 dark:bg-slate-800 dark:text-slate-300">
                  {m.mount_type}
                </span>
                <span class="font-mono text-[11px] text-slate-400">{m.mode || "rw"}</span>
              </div>
              <div class="mt-2.5 space-y-1 font-mono text-xs text-slate-700 dark:text-slate-300">
                <div class="flex items-center justify-between gap-2">
                  <div class="min-w-0 break-all"><span class="text-slate-400">Src:</span> {m.source}</div>
                  <button
                    type="button"
                    onclick={() => copyText(`src:${m.source}`, m.source)}
                    title="Copy source path"
                    aria-label="Copy mount source path"
                    class="shrink-0 rounded p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-slate-800"
                  >
                    <Icon name={copiedKey === `src:${m.source}` ? "check" : "copy"} class="h-3.5 w-3.5" />
                  </button>
                </div>
                <div class="break-all"><span class="text-slate-400">Dst:</span> {m.destination}</div>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  {/if}
  {#if activeTab === "terminal"}
    <ContainerTerminalTab {containerId} {isRunning} />
  {:else if activeTab === "files"}
    <ContainerFilesTab {containerId} />
  {/if}
</section>

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
