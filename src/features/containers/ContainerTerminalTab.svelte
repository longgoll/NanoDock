<script lang="ts">
  import { daemonApi } from "../../lib/api/daemon";
  import Icon from "../../lib/components/Icon.svelte";
  import { i18n } from "../../lib/i18n/index.svelte";

  let {
    containerId,
    isRunning,
  }: {
    containerId: string;
    isRunning: boolean;
  } = $props();

  interface HistoryEntry {
    cmd: string;
    exitCode: number;
    output: string;
  }

  let commandInput = $state("");
  let running = $state(false);
  let history = $state<HistoryEntry[]>([]);
  let error = $state<string | null>(null);
  let outputBox = $state<HTMLDivElement | null>(null);

  const presets = ["ls -la", "env", "ps aux", "df -h", "cat /etc/os-release"];

  function parseCommandArgs(raw: string): string[] {
    const trimmed = raw.trim();
    if (!trimmed) return [];
    return ["sh", "-c", trimmed];
  }

  async function runCommand(cmdText?: string) {
    const text = (cmdText ?? commandInput).trim();
    if (!text || !isRunning || running) return;
    running = true;
    error = null;
    try {
      const resp = await daemonApi.execContainer(containerId, parseCommandArgs(text));
      if (resp.status === "exec_output") {
        history = [
          ...history,
          { cmd: text, exitCode: resp.data.exit_code, output: resp.data.output },
        ];
        if (!cmdText) commandInput = "";
        queueMicrotask(() => {
          if (outputBox) outputBox.scrollTop = outputBox.scrollHeight;
        });
      } else if (resp.status === "error") {
        error = resp.data.message;
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      running = false;
    }
  }
</script>

<div class="flex flex-col gap-3">
  {#if !isRunning}
    <div class="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-xs text-amber-800 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300">
      {i18n.t.detail.execNotRunning}
    </div>
  {:else}
    <div class="flex flex-wrap items-center gap-1.5">
      {#each presets as preset}
        <button
          type="button"
          onclick={() => runCommand(preset)}
          disabled={running}
          class="rounded border border-slate-200 bg-white px-2.5 py-1 font-mono text-[11px] text-slate-700 transition hover:border-sky-400 hover:text-sky-600 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800/70 dark:text-slate-300 dark:hover:border-sky-500/50 dark:hover:text-sky-300"
        >
          {preset}
        </button>
      {/each}
      {#if history.length > 0}
        <button
          type="button"
          onclick={() => (history = [])}
          class="ml-auto rounded border border-slate-200 px-2.5 py-1 text-[11px] text-slate-600 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
        >
          {i18n.t.detail.clearLogs}
        </button>
      {/if}
    </div>

    <form
      onsubmit={(e) => {
        e.preventDefault();
        runCommand();
      }}
      class="flex gap-2"
    >
      <div class="relative flex-1">
        <span class="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 font-mono text-xs text-sky-600 dark:text-sky-400">$</span>
        <input
          type="text"
          bind:value={commandInput}
          placeholder={i18n.t.detail.execPlaceholder}
          disabled={running}
          class="w-full rounded-lg border border-slate-200 bg-white py-2 pr-3 pl-7 font-mono text-xs text-slate-900 placeholder-slate-400 focus:border-sky-500 focus:outline-none dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100 dark:placeholder-slate-500"
        />
      </div>
      <button
        type="submit"
        disabled={running || !commandInput.trim()}
        class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3.5 py-2 text-xs font-medium text-white transition hover:bg-sky-500 disabled:opacity-50"
      >
        <Icon name="terminal" class="h-3.5 w-3.5" />
        {i18n.t.detail.execRun}
      </button>
    </form>

    {#if error}
      <div class="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-xs text-rose-700 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-300">
        {error}
      </div>
    {/if}

    <div
      bind:this={outputBox}
      class="max-h-96 overflow-y-auto rounded-xl border border-slate-200 bg-slate-950 p-3.5 font-mono text-xs shadow-xs dark:border-slate-800"
    >
      {#if history.length === 0}
        <p class="text-slate-500">$ _</p>
      {:else}
        {#each history as item}
          <div class="mb-3 border-b border-slate-800/80 pb-2 last:mb-0 last:border-b-0 last:pb-0">
            <div class="flex items-center justify-between text-sky-400">
              <span>$ {item.cmd}</span>
              <span class="text-[10px] {item.exitCode === 0 ? 'text-emerald-400' : 'text-rose-400'}">
                exit {item.exitCode}
              </span>
            </div>
            {#if item.output}
              <pre class="mt-1 whitespace-pre-wrap break-all text-slate-200">{item.output}</pre>
            {/if}
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</div>
