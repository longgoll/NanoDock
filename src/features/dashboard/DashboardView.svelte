<script lang="ts">
  import { onMount } from "svelte";
  import HelpTip from "../../lib/components/HelpTip.svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import RefreshButton from "../../lib/components/RefreshButton.svelte";
  import StatusBadge from "../../lib/components/StatusBadge.svelte";
  import Tooltip from "../../lib/components/Tooltip.svelte";
  import { daemonApi, type EngineMetrics } from "../../lib/api/daemon";
  import { i18n } from "../../lib/i18n/index.svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";
  import { formatBytes } from "../../lib/utils/format";
  import DiagnosticsModal from "../settings/DiagnosticsModal.svelte";

  type NavTarget = "containers" | "images" | "volumes" | "storage";

  let { onNavigate }: { onNavigate?: (tab: NavTarget) => void } = $props();

  let copiedKey = $state<"pipe" | "cli" | null>(null);
  let metrics = $state<EngineMetrics | null>(null);
  let diagnosticsOpen = $state(false);

  const isStopped = $derived(engineStore.state === "stopped");

  onMount(() => {
    if (!engineStore.config) {
      void engineStore.loadConfig();
    }
    void daemonApi.getEngineMetrics().then((res) => {
      if (res.status === "engine_metrics") metrics = res.data;
    }).catch(() => {});
  });

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
    if (secs === null || secs === undefined) return i18n.t.dashboard.never;
    if (secs < 60) return `${secs}s`;
    return `${Math.round(secs / 60)} min`;
  }
</script>

<section class="space-y-6">
  <div
    class="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-slate-200 bg-white p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/60"
  >
    <div class="space-y-2.5">
      <div class="flex items-center gap-2">
        <h2 class="text-lg font-semibold text-slate-900 dark:text-white">
          {i18n.t.dashboard.title}
        </h2>
        <HelpTip
          title={i18n.t.dashboard.helpTitle}
          align="left"
          position="bottom"
          text={i18n.t.dashboard.helpText}
        />
      </div>
      <div class="flex flex-wrap items-center gap-2.5">
        <StatusBadge state={engineStore.state} />
        <span class="text-xs text-slate-500 dark:text-slate-400">
          {isStopped ? i18n.t.dashboard.sleepingHint : i18n.t.dashboard.runningHint}
        </span>
      </div>
    </div>

    <div class="flex gap-3">
      <RefreshButton
        size="md"
        onRefresh={() => engineStore.refreshAll(true)}
        disabled={engineStore.busy}
      />

      {#if engineStore.state === "stopped"}
        <button
          type="button"
          onclick={() => engineStore.start()}
          disabled={engineStore.busy}
          class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-4 py-2 text-sm font-medium text-white shadow-xs transition hover:bg-sky-500 disabled:opacity-50"
        >
          <Icon
            name={engineStore.isPending("engine:start") ? "spinner" : "play"}
            class={`h-4 w-4 ${engineStore.isPending("engine:start") ? "animate-spin" : ""}`}
          />
          {engineStore.isPending("engine:start") ? i18n.t.dashboard.starting : i18n.t.dashboard.startEngine}
        </button>
      {:else}
        <button
          type="button"
          onclick={() => engineStore.stop()}
          disabled={engineStore.busy}
          class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-slate-100 px-4 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-200 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
        >
          <Icon
            name={engineStore.isPending("engine:stop") ? "spinner" : "stop"}
            class={`h-4 w-4 ${engineStore.isPending("engine:stop") ? "animate-spin" : ""}`}
          />
          {engineStore.isPending("engine:stop") ? i18n.t.status.stopping : i18n.t.dashboard.hibernate}
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
        {i18n.t.dashboard.runningContainers}
      </p>
      <p class="mt-2 text-3xl font-bold text-slate-900 dark:text-white">
        {isStopped ? "—" : engineStore.runningContainers}
      </p>
      <p class="mt-1 text-xs text-slate-400">
        {isStopped
          ? i18n.t.dashboard.engineSleeping
          : i18n.t.dashboard.totalRegistered(engineStore.containers.length)}
      </p>
    </button>

    <button
      type="button"
      onclick={() => onNavigate?.("storage")}
      class="rounded-2xl border border-slate-200 bg-white p-5 text-left shadow-xs transition hover:border-sky-300 dark:border-slate-800 dark:bg-slate-900/40 dark:hover:border-sky-500/40"
    >
      <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
        {i18n.t.dashboard.vhdxSize}
      </p>
      <p class="mt-2 text-3xl font-bold text-slate-900 dark:text-white">
        {formatBytes(engineStore.vhdxSizeBytes)}
      </p>
      <p class="mt-1 text-xs text-slate-400">{i18n.t.dashboard.sparseDisk}</p>
    </button>

    <button
      type="button"
      onclick={() => onNavigate?.("images")}
      class="rounded-2xl border border-slate-200 bg-white p-5 text-left shadow-xs transition hover:border-sky-300 dark:border-slate-800 dark:bg-slate-900/40 dark:hover:border-sky-500/40"
    >
      <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
        {i18n.t.dashboard.localImages}
      </p>
      <p class="mt-2 text-3xl font-bold text-slate-900 dark:text-white">
        {isStopped ? "—" : engineStore.images.length}
      </p>
      <p class="mt-1 text-xs text-slate-400">
        {isStopped
          ? i18n.t.dashboard.engineSleeping
          : i18n.t.dashboard.unusedCount(engineStore.images.filter((i) => !i.in_use).length)}
      </p>
    </button>

    <button
      type="button"
      onclick={() => onNavigate?.("volumes")}
      class="rounded-2xl border border-slate-200 bg-white p-5 text-left shadow-xs transition hover:border-sky-300 dark:border-slate-800 dark:bg-slate-900/40 dark:hover:border-sky-500/40"
    >
      <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
        {i18n.t.dashboard.volumes}
      </p>
      <p class="mt-2 text-3xl font-bold text-slate-900 dark:text-white">
        {isStopped ? "—" : engineStore.volumes.length}
      </p>
      <p class="mt-1 text-xs text-slate-400">
        {isStopped
          ? i18n.t.dashboard.engineSleeping
          : i18n.t.dashboard.mountedCount(engineStore.volumes.filter((v) => v.in_use).length)}
      </p>
    </button>
  </div>

  {#if engineStore.config}
    <div
      class="grid grid-cols-2 gap-4 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs sm:grid-cols-4 dark:border-slate-800 dark:bg-slate-900/40"
    >
      <div>
        <p class="text-[11px] font-medium uppercase tracking-wider text-slate-400">
          {i18n.t.dashboard.ramLimit}
        </p>
        <p class="mt-1 font-mono text-sm font-semibold text-slate-900 dark:text-white">
          {engineStore.config.wsl.memory_mb} MB
        </p>
      </div>
      <div>
        <p class="text-[11px] font-medium uppercase tracking-wider text-slate-400">
          {i18n.t.dashboard.swap}
        </p>
        <p class="mt-1 font-mono text-sm font-semibold text-slate-900 dark:text-white">
          {engineStore.config.wsl.swap_mb} MB
        </p>
      </div>
      <div>
        <div class="flex items-center gap-1">
          <p class="text-[11px] font-medium uppercase tracking-wider text-slate-400">
            {i18n.t.dashboard.autoHibernate}
          </p>
          <HelpTip text={i18n.t.dashboard.autoHibernateHelp} />
        </div>
        <p class="mt-1 text-sm font-semibold text-slate-900 dark:text-white">
          {formatIdleTimeout(engineStore.config.idle_timeout_secs)}
        </p>
      </div>
      <div>
        <div class="flex items-center gap-1">
          <p class="text-[11px] font-medium uppercase tracking-wider text-slate-400">
            {i18n.t.dashboard.sparseVhd}
          </p>
          <HelpTip align="right" text={i18n.t.dashboard.sparseVhdHelp} />
        </div>
        <p class="mt-1 text-sm font-semibold text-slate-900 dark:text-white">
          {engineStore.config.wsl.sparse_vhd ? i18n.t.dashboard.enabled : i18n.t.dashboard.disabled}
        </p>
      </div>
    </div>
  {/if}

  <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
    <div
      class="flex items-start justify-between gap-3 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40"
    >
      <div class="min-w-0">
        <div class="flex items-center gap-1.5">
          <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
            {i18n.t.dashboard.namedPipe}
          </p>
          <HelpTip
            title={i18n.t.dashboard.namedPipeHelpTitle}
            align="left"
            text={i18n.t.dashboard.namedPipeHelpText}
          />
        </div>
        <p class="mt-2 truncate font-mono text-sm font-medium text-sky-700 dark:text-sky-400">
          \\.\pipe\docker_engine
        </p>
      </div>
      <Tooltip text={copiedKey === "pipe" ? i18n.t.common.copied : i18n.t.dashboard.copyEndpoint} align="right">
        <button
          type="button"
          onclick={() => copyText("pipe", "\\\\.\\pipe\\docker_engine")}
          aria-label={i18n.t.dashboard.copyEndpoint}
          class="shrink-0 rounded-lg border border-slate-200 p-2 text-slate-500 transition hover:bg-slate-100 hover:text-slate-700 dark:border-slate-700 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
        >
          <Icon name={copiedKey === "pipe" ? "check" : "copy"} class="h-3.5 w-3.5" />
        </button>
      </Tooltip>
    </div>

    <div
      class="flex items-start justify-between gap-3 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40"
    >
      <div class="min-w-0">
        <div class="flex items-center gap-1.5">
          <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
            {i18n.t.dashboard.cliUsage}
          </p>
          <HelpTip text={i18n.t.dashboard.cliUsageHelp} />
        </div>
        <p class="mt-2 truncate font-mono text-sm font-medium text-slate-700 dark:text-slate-300">
          nanodock run / docker run / docker compose up -d
        </p>
      </div>
      <Tooltip text={copiedKey === "cli" ? i18n.t.common.copied : i18n.t.dashboard.copySampleCmd} align="right">
        <button
          type="button"
          onclick={() => copyText("cli", "docker compose up -d")}
          aria-label={i18n.t.dashboard.copySampleCmd}
          class="shrink-0 rounded-lg border border-slate-200 p-2 text-slate-500 transition hover:bg-slate-100 hover:text-slate-700 dark:border-slate-700 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
        >
          <Icon name={copiedKey === "cli" ? "check" : "copy"} class="h-3.5 w-3.5" />
        </button>
      </Tooltip>
    </div>
  </div>

  <!-- Resource Savings & Diagnostics -->
  <div class="flex flex-wrap items-center justify-between gap-4 rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40">
    <div class="flex flex-wrap items-center gap-6">
      <div>
        <p class="text-[11px] font-medium uppercase tracking-wider text-slate-400">
          {i18n.t.extra.hibernateCount}
        </p>
        <p class="mt-1 text-lg font-bold text-slate-900 dark:text-white">
          {metrics?.hibernate_count ?? 0}
        </p>
      </div>
      <div>
        <p class="text-[11px] font-medium uppercase tracking-wider text-slate-400">
          {i18n.t.extra.reclaimedDisk}
        </p>
        <p class="mt-1 text-lg font-bold text-emerald-600 dark:text-emerald-400">
          {formatBytes(metrics?.total_reclaimed_bytes ?? 0)}
        </p>
      </div>
      <div>
        <p class="text-[11px] font-medium uppercase tracking-wider text-slate-400">
          {i18n.t.extra.uptime}
        </p>
        <p class="mt-1 text-lg font-bold text-slate-900 dark:text-white">
          {metrics ? `${Math.floor(metrics.uptime_secs / 60)}m` : "—"}
        </p>
      </div>
    </div>

    <button
      type="button"
      onclick={() => (diagnosticsOpen = true)}
      class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-slate-50 px-3.5 py-2 text-xs font-medium text-slate-700 transition hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
    >
      <Icon name="activity" class="h-3.5 w-3.5 text-sky-500" />
      {i18n.t.extra.runDoctor}
    </button>
  </div>
</section>

<DiagnosticsModal open={diagnosticsOpen} onClose={() => (diagnosticsOpen = false)} />
