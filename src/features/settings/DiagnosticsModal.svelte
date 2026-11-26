<script lang="ts">
  import {
    daemonApi,
    systemApi,
    type DoctorCheckItem,
  } from "../../lib/api/daemon";
  import Icon from "../../lib/components/Icon.svelte";
  import { i18n } from "../../lib/i18n/index.svelte";

  let {
    open,
    onClose,
  }: {
    open: boolean;
    onClose: () => void;
  } = $props();

  let checks = $state<DoctorCheckItem[]>([]);
  let daemonLog = $state("");
  let dockerdLog = $state("");
  let loading = $state(false);
  let activeLogTab = $state<"daemon" | "dockerd">("daemon");

  export async function refreshDiagnostics() {
    loading = true;
    try {
      const [doctorRes, logsRes] = await Promise.all([
        systemApi.runDoctorChecks(),
        daemonApi.getEngineLogs(150).catch(() => null),
      ]);
      checks = doctorRes;
      if (logsRes && logsRes.status === "engine_logs") {
        daemonLog = logsRes.data.daemon_log;
        dockerdLog = logsRes.data.dockerd_log;
      }
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (open) {
      void refreshDiagnostics();
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (open && e.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
  <div
    onclick={(e) => {
      if (e.target === e.currentTarget) onClose();
    }}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    aria-label={i18n.t.extra.diagnosticsTitle}
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/60 p-4 backdrop-blur-xs"
  >
    <div class="flex max-h-[88vh] w-full max-w-4xl flex-col rounded-2xl border border-slate-200 bg-white shadow-2xl dark:border-slate-800 dark:bg-slate-900">
      <div class="flex items-center justify-between border-b border-slate-200 px-5 py-3.5 dark:border-slate-800">
        <h3 class="text-sm font-semibold text-slate-900 dark:text-slate-100">{i18n.t.extra.diagnosticsTitle}</h3>
        <div class="flex items-center gap-2">
          <button
            type="button"
            onclick={refreshDiagnostics}
            disabled={loading}
            aria-label={i18n.t.common.refresh}
            class="rounded-lg border border-slate-200 px-2.5 py-1 text-xs text-slate-600 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
          >
            <Icon name="refresh" class={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
          </button>
          <button
            type="button"
            onclick={onClose}
            aria-label={i18n.t.common.cancel}
            class="rounded-lg p-1.5 text-slate-400 hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-slate-800 dark:hover:text-slate-100"
          >
            <Icon name="close" class="h-4 w-4" />
          </button>
        </div>
      </div>

      <div class="flex-1 space-y-4 overflow-y-auto p-5">
        <div class="grid grid-cols-1 gap-2 sm:grid-cols-2">
          {#each checks as item}
            <div class="flex items-start gap-2.5 rounded-lg border border-slate-200 bg-slate-50 p-3 dark:border-slate-800 dark:bg-slate-950/70">
              <span
                class="mt-0.5 rounded px-1.5 py-0.5 font-mono text-[10px] font-bold uppercase {item.level === 'ok'
                  ? 'bg-emerald-100 text-emerald-700 dark:bg-emerald-500/20 dark:text-emerald-400'
                  : item.level === 'warn'
                    ? 'bg-amber-100 text-amber-700 dark:bg-amber-500/20 dark:text-amber-400'
                    : 'bg-rose-100 text-rose-700 dark:bg-rose-500/20 dark:text-rose-400'}"
              >
                {item.level}
              </span>
              <div class="min-w-0">
                <div class="text-xs font-medium text-slate-800 dark:text-slate-200">{item.name}</div>
                <div class="break-words text-[11px] text-slate-500 dark:text-slate-400">{item.detail}</div>
              </div>
            </div>
          {/each}
        </div>

        <div>
          <div class="mb-2 flex items-center gap-2">
            <button
              type="button"
              onclick={() => (activeLogTab = "daemon")}
              class="rounded-md px-3 py-1 text-xs font-medium {activeLogTab === 'daemon'
                ? 'bg-sky-600 text-white'
                : 'bg-slate-100 text-slate-600 hover:bg-slate-200 dark:bg-slate-800 dark:text-slate-300'}"
            >
              core-daemon.log
            </button>
            <button
              type="button"
              onclick={() => (activeLogTab = "dockerd")}
              class="rounded-md px-3 py-1 text-xs font-medium {activeLogTab === 'dockerd'
                ? 'bg-sky-600 text-white'
                : 'bg-slate-100 text-slate-600 hover:bg-slate-200 dark:bg-slate-800 dark:text-slate-300'}"
            >
              dockerd.log (WSL2)
            </button>
          </div>
          <pre class="max-h-72 overflow-y-auto rounded-lg border border-slate-200 bg-slate-950 p-3 font-mono text-[11px] whitespace-pre-wrap break-all text-slate-200 dark:border-slate-800">{activeLogTab === "daemon"
              ? daemonLog || "(empty)"
              : dockerdLog || "(empty or engine sleeping)"}</pre>
        </div>
      </div>
    </div>
  </div>
{/if}
