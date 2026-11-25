<script lang="ts">
  import StatusBadge from "../../lib/components/StatusBadge.svelte";
  import { engineStore } from "../../lib/stores/engine.svelte";
</script>

<section class="space-y-6">
  <div class="flex items-center justify-between rounded-2xl border border-slate-200 bg-white p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
    <div class="space-y-2.5">
      <h2 class="text-lg font-semibold text-slate-900 dark:text-white">WSL2 Micro-Distro Engine</h2>
      <StatusBadge state={engineStore.state} />
    </div>

    <div class="flex gap-3">
      <button
        onclick={() => engineStore.refresh()}
        disabled={engineStore.busy}
        class="rounded-lg border border-slate-200 bg-white px-3.5 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
      >
        Refresh
      </button>

      {#if engineStore.state === "stopped"}
        <button
          onclick={() => engineStore.start()}
          disabled={engineStore.busy}
          class="rounded-lg bg-sky-600 px-4 py-2 text-sm font-medium text-white shadow-xs transition hover:bg-sky-500 disabled:opacity-50"
        >
          Start Engine
        </button>
      {:else}
        <button
          onclick={() => engineStore.stop()}
          disabled={engineStore.busy}
          class="rounded-lg border border-slate-200 bg-slate-100 px-4 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-200 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
        >
          Hibernate (0 MB)
        </button>
      {/if}
    </div>
  </div>

  <div class="grid grid-cols-1 gap-4 md:grid-cols-3">
    <div class="rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40">
      <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
        Active Streams
      </p>
      <p class="mt-2 text-3xl font-bold text-slate-900 dark:text-white">
        {engineStore.runningContainers}
      </p>
    </div>

    <div class="rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40">
      <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
        Named Pipe Endpoint
      </p>
      <p class="mt-2 font-mono text-sm font-medium text-sky-700 dark:text-sky-400">
        \\.\pipe\docker_engine
      </p>
    </div>

    <div class="rounded-2xl border border-slate-200 bg-white p-5 shadow-xs dark:border-slate-800 dark:bg-slate-900/40">
      <p class="text-xs font-medium uppercase tracking-wider text-slate-500 dark:text-slate-400">
        CLI Usage
      </p>
      <p class="mt-2 font-mono text-sm font-medium text-slate-700 dark:text-slate-300">
        nanodock run / docker run
      </p>
    </div>
  </div>
</section>
