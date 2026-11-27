<script lang="ts">
  import { Channel } from "@tauri-apps/api/core";
  import { FitAddon } from "@xterm/addon-fit";
  import { Terminal } from "@xterm/xterm";
  import "@xterm/xterm/css/xterm.css";
  import { onDestroy } from "svelte";
  import { daemonApi, type ExecServerFrame } from "../../lib/api/daemon";
  import Icon from "../../lib/components/Icon.svelte";
  import { i18n } from "../../lib/i18n/index.svelte";

  let {
    containerId,
    isRunning,
  }: {
    containerId: string;
    isRunning: boolean;
  } = $props();

  let terminalHost = $state<HTMLDivElement | null>(null);
  let selectedShell = $state("/bin/sh");
  let status = $state<"idle" | "connecting" | "connected" | "exited" | "error">("idle");
  let errorMsg = $state<string | null>(null);
  let sessionId = $state<string | null>(null);

  let term: Terminal | null = null;
  let fitAddon: FitAddon | null = null;
  let resizeObserver: ResizeObserver | null = null;

  async function disconnect() {
    if (sessionId) {
      const id = sessionId;
      sessionId = null;
      await daemonApi.closeExecSession(id).catch(() => {});
    }
  }

  async function connect() {
    if (!isRunning || !terminalHost) return;
    await disconnect();
    status = "connecting";
    errorMsg = null;

    if (!term) {
      term = new Terminal({
        cursorBlink: true,
        fontSize: 12,
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
        theme: {
          background: "#020617",
          foreground: "#e2e8f0",
          cursor: "#38bdf8",
        },
        scrollback: 3000,
      });
      fitAddon = new FitAddon();
      term.loadAddon(fitAddon);
      term.open(terminalHost);
      fitAddon.fit();

      term.onData((data) => {
        if (sessionId) {
          void daemonApi.sendExecInput(sessionId, data);
        }
      });

      resizeObserver = new ResizeObserver(() => {
        if (!fitAddon || !term) return;
        fitAddon.fit();
        if (sessionId) {
          void daemonApi.resizeExecSession(sessionId, term.cols, term.rows);
        }
      });
      resizeObserver.observe(terminalHost);
    } else {
      fitAddon?.fit();
      term.clear();
    }

    const cols = term.cols || 80;
    const rows = term.rows || 24;
    const channel = new Channel<ExecServerFrame>();
    channel.onmessage = (frame) => {
      if (frame.type === "started") {
        sessionId = frame.exec_id;
        status = "connected";
        term?.focus();
      } else if (frame.type === "output") {
        term?.write(frame.data);
      } else if (frame.type === "exit") {
        status = "exited";
        sessionId = null;
        term?.writeln(`\r\n[process exited with code ${frame.code}]`);
      } else if (frame.type === "error") {
        status = "error";
        errorMsg = frame.message;
        sessionId = null;
      }
    };

    try {
      sessionId = await daemonApi.startExecSession(
        containerId,
        selectedShell,
        cols,
        rows,
        channel
      );
      status = "connected";
    } catch (e) {
      status = "error";
      errorMsg = e instanceof Error ? e.message : String(e);
    }
  }

  $effect(() => {
    if (isRunning && terminalHost) {
      void connect();
    } else {
      void disconnect();
      status = "idle";
    }
  });

  onDestroy(() => {
    void disconnect();
    resizeObserver?.disconnect();
    term?.dispose();
  });
</script>

<div class="flex flex-col gap-3">
  {#if !isRunning}
    <div class="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-xs text-amber-800 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-300">
      {i18n.t.detail.execNotRunning}
    </div>
  {:else}
    <div class="flex flex-wrap items-center gap-2">
      <select
        bind:value={selectedShell}
        onchange={() => connect()}
        class="rounded-lg border border-slate-200 bg-white px-2.5 py-1.5 font-mono text-xs text-slate-700 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200"
      >
        <option value="/bin/sh">/bin/sh</option>
        <option value="/bin/bash">/bin/bash</option>
        <option value="/bin/ash">/bin/ash</option>
      </select>
      <button
        type="button"
        onclick={() => connect()}
        class="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-2.5 py-1.5 text-xs font-medium text-slate-700 transition hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
      >
        <Icon name="refresh" class="h-3.5 w-3.5" />
        Reconnect
      </button>
      <button
        type="button"
        onclick={() => term?.clear()}
        class="rounded-lg border border-slate-200 px-2.5 py-1.5 text-xs text-slate-600 hover:bg-slate-100 dark:border-slate-700 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-slate-200"
      >
        {i18n.t.detail.clearLogs}
      </button>
      <span class="ml-auto font-mono text-[11px] text-slate-500 dark:text-slate-400">
        {status}
      </span>
    </div>

    {#if errorMsg}
      <div class="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-xs text-rose-700 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-300">
        {errorMsg}
      </div>
    {/if}

    <div
      bind:this={terminalHost}
      class="h-[360px] w-full overflow-hidden rounded-lg border border-slate-200 bg-slate-950 p-2 dark:border-slate-800"
    ></div>
  {/if}
</div>
