<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import StatusBadge from "../../lib/components/StatusBadge.svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";
  import { formatBytes } from "../../lib/utils/format";

  type NavTarget = "containers" | "images" | "volumes" | "storage";

  let { onNavigate }: { onNavigate?: (tab: NavTarget) => void } = $props();

  let refreshing = $state(false);
  let copiedKey = $state<"pipe" | "cli" | null>(null);

  const isStopped = $derived(engineStore.state === "stopped");

  onMount(() => {
    if (!engineStore.config) {
      void engineStore.loadConfig();
    }
  });

  async function refreshDashboard() {
    refreshing = true;
    try {
      await engineStore.refreshAll(true);
    } finally {
      refreshing = false;
    }
  }

  async function copyText(key: "pipe" | "cli", text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedKey = key;
      setTimeout(() => {
        if (copiedKey === key) copiedKey = null;
      }, 1400);
    } catch {
      // ignore clipboard errors
    }
  }

  function formatIdleTimeout(secs: number | null | undefined): string {
    if (secs === null || secs === undefined) return "Never";
    if (secs < 60) return `${secs}s`;
    return `${Math.round(secs / 60)} min`;
  }
</script>

<section class="space-y-6">
  <div
    class="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-slate-200 bg-white p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/60"
  >
    <div class="space-y-2.5">
      <h2 class="text-lg font-semibold text-slate-900 dark:text-white">
        WSL2 Micro-Distro Engine
      </h2>
      <StatusBadge state={engineStore.state} />
    </div>

    <div class="flex gap-3">
      <button
        type="button"
        onclick={refreshDashboard}
        disabled={engineStore.busy || refreshing}
        class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-3.5 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
      >
        <Icon name="refresh" class={`h-4 w-4 ${refreshing ? "animate-spin" : ""}`} />
        Refresh
      </button>

      {#if engineStore.state === "stopped"}
        <button
          type="button"
          onclick={() => engineStore.start()}
          disabled={engineStore.busy}
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-sm font-medium text-white shadow-xs transition hover:bg-sky-500 disabled:opacity-50"
        >
          <Icon name="play" class="h-4 w-4" />
          {engineStore.busy ? "Starting..." : "Start Engine"}
        </button>
      {:else}
        <button
          type="button"
          onclick={() => engineStore.stop()}
          disabled={engineStore.busy}
          class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-slate-100 px-4 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-200 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
        >
          <Icon name="stop" class="h-4 w-4" />
          Hibernate (0 MB)
        </button>
      {/if}
    </div>
  </div>

  <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
    <button
      type="button"
      onclick={() => onNavigate?.("containers")}
      class="rounded-2xl border border-slate-200 bg-white p-5 text-left shadow-xs transition hover:border-sky-300 dark:border-slate-800 dark:bg-slate-900/40 dark:hover:border-sky-500/40"
    >
      <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
        Running Containers
      </p>
      <p class="mt-2 text-3xl font-bold text-slate-900 dark:text-white">
        {isStopped ? "—" : engineStore.runningContainers}
      </p>
      <p class="mt-1 text-xs text-slate-400">
        {isStopped ? "Engine sleeping" : `${engineStore.containers.length} total registered`}
      </p>
    </button>

    <button
      type="button"
      onclick={() => onNavigate?.("storage")}
      class="rounded-2xl border border-slate-200 bg-white p-5 text-left shadow-xs transition hover:border-sky-300 dark:border-slate-800 dark:bg-slate-900/40 dark:hover:border-sky-500/40"
    >
      <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
        VHDX Disk Size
      </p>
      <p class="mt-2 text-3xl font-bold text-slate-900 dark:text-white">
        {formatBytes(engineStore.vhdxSizeBytes)}
      </p>
      <p class="mt-1 text-xs text-slate-400">Sparse WSL2 virtual disk</p>
    </button>

    <button
      type="button"
      onclick={() => onNavigate?.("images")}
      class="rounded-2xl border border-slate-200 bg-white p-5 text-left shadow-xs transition hover:border-sky-300 dark:border-slate-800 dark:bg-slate-900/40 dark:hover:border-sky-500/40"
    >
      <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
        Local Images
      </p>
      <p class="mt-2 text-3xl font-bold text-slate-900 dark:text-white">
        {isStopped ? "—" : engineStore.images.length}
      </p>
      <p class="mt-1 text-xs text-slate-400">
        {isStopped
          ? "Engine sleeping"
          : `${engineStore.images.filter((i) => !i.in_use).length} unused`}
      </p>
    </button>

    <button
      type="button"
      onclick={() => onNavigate?.("volumes")}
      class="rounded-2xl border border-slate-200 bg-white p-5 text-left shadow-xs transition hover:border-sky-300 dark:border-slate-800 dark:bg-slate-900/40 dark:hover:border-sky-500/40"
    >
      <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
        Volumes
      </p>
      <p class="mt-2 text-3xl font-bold text-slate-900 dark:text-white">
        {isStopped ? "—" : engineStore.volumes.length}
      </p>
      <p class="mt-1 text-xs text-slate-400">
        {isStopped
          ? "Engine sleeping"
          : `${engineStore.volumes.filter((v) => v.in_use).length} mounted`}
      </p>
    </button>
  </div>

  {#if engineStore.config}
    <div
      class="grid grid-cols-2 gap-4 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs sm:grid-cols-4 dark:border-slate-800 dark:bg-slate-900/40"
    >
      <div>
        <p class="text-[11px] font-medium uppercase tracking-wider text-slate-400">RAM Limit</p>
        <p class="mt-1 font-mono text-sm font-semibold text-slate-900 dark:text-white">
          {engineStore.config.wsl.memory_mb} MB
        </p>
      </div>
      <div>
        <p class="text-[11px] font-medium uppercase tracking-wider text-slate-400">Swap</p>
        <p class="mt-1 font-mono text-sm font-semibold text-slate-900 dark:text-white">
          {engineStore.config.wsl.swap_mb} MB
        </p>
      </div>
      <div>
        <p class="text-[11px] font-medium uppercase tracking-wider text-slate-400">Auto-Hibernate</p>
        <p class="mt-1 text-sm font-semibold text-slate-900 dark:text-white">
          {formatIdleTimeout(engineStore.config.idle_timeout_secs)}
        </p>
      </div>
      <div>
        <p class="text-[11px] font-medium uppercase tracking-wider text-slate-400">Sparse VHD</p>
        <p class="mt-1 text-sm font-semibold text-slate-900 dark:text-white">
          {engineStore.config.wsl.sparse_vhd ? "Enabled" : "Disabled"}
        </p>
      </div>
    </div>
  {/if}

  <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
    <div
      class="flex items-start justify-between gap-3 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40"
    >
      <div class="min-w-0">
        <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
          Named Pipe Endpoint
        </p>
        <p class="mt-2 truncate font-mono text-sm font-medium text-sky-700 dark:text-sky-400">
          \\.\pipe\docker_engine
        </p>
      </div>
      <button
        type="button"
        onclick={() => copyText("pipe", "\\\\.\\pipe\\docker_engine")}
        title="Copy endpoint"
        aria-label="Copy named pipe endpoint"
        class="shrink-0 rounded-lg border border-slate-200 p-2 text-slate-500 transition hover:bg-slate-100 hover:text-slate-700 dark:border-slate-700 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
      >
        <Icon name={copiedKey === "pipe" ? "check" : "copy"} class="h-3.5 w-3.5" />
      </button>
    </div>

    <div
      class="flex items-start justify-between gap-3 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40"
    >
      <div class="min-w-0">
        <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
          CLI Usage
        </p>
        <p class="mt-2 truncate font-mono text-sm font-medium text-slate-700 dark:text-slate-300">
          nanodock run / docker run / docker compose up -d
        </p>
      </div>
      <button
        type="button"
        onclick={() => copyText("cli", "docker compose up -d")}
        title="Copy sample command"
        aria-label="Copy sample CLI command"
        class="shrink-0 rounded-lg border border-slate-200 p-2 text-slate-500 transition hover:bg-slate-100 hover:text-slate-700 dark:border-slate-700 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
      >
        <Icon name={copiedKey === "cli" ? "check" : "copy"} class="h-3.5 w-3.5" />
      </button>
    </div>
  </div>
</section>
