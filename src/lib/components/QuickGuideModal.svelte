<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    open,
    onClose,
  }: {
    open: boolean;
    onClose: () => void;
  } = $props();

  function handleKeydown(e: KeyboardEvent) {
    if (open && e.key === "Escape") {
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/60 p-4 backdrop-blur-xs"
    onclick={(e) => {
      if (e.target === e.currentTarget) onClose();
    }}
  >
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="quick-guide-title"
      class="w-full max-w-2xl rounded-2xl border border-slate-200 bg-white p-6 shadow-2xl dark:border-slate-800 dark:bg-slate-900"
    >
      <div class="flex items-start justify-between gap-4 border-b border-slate-200 pb-4 dark:border-slate-800">
        <div class="flex items-center gap-3">
          <div class="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-sky-50 text-sky-600 dark:bg-sky-500/15 dark:text-sky-400">
            <Icon name="help" class="h-5 w-5" />
          </div>
          <div>
            <h2 id="quick-guide-title" class="text-base font-semibold text-slate-900 dark:text-white">
              NanoDock Quick Guide
            </h2>
            <p class="text-xs text-slate-500 dark:text-slate-400">
              How NanoDock runs containers lightly on Windows + WSL2
            </p>
          </div>
        </div>
        <button
          type="button"
          onclick={onClose}
          aria-label="Close quick guide"
          class="rounded-lg p-1.5 text-slate-400 transition hover:bg-slate-100 hover:text-slate-600 dark:hover:bg-slate-800 dark:hover:text-slate-200"
        >
          <Icon name="close" class="h-4 w-4" />
        </button>
      </div>

      <div class="mt-5 grid grid-cols-1 gap-3.5 sm:grid-cols-2">
        <div class="rounded-xl border border-slate-200 bg-slate-50/70 p-4 dark:border-slate-800 dark:bg-slate-800/40">
          <p class="text-xs font-semibold text-slate-900 dark:text-white">
            1. Auto-Sleep (0 MB Idle RAM)
          </p>
          <p class="mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-slate-300">
            When no containers or log streams are active for the configured <strong>Idle Timeout</strong>, NanoDock safely shuts down the WSL2 micro-distro to free all RAM. Running any <code class="rounded bg-slate-200/70 px-1 py-0.5 font-mono text-[11px] dark:bg-slate-700">docker</code> command wakes it automatically.
          </p>
        </div>

        <div class="rounded-xl border border-slate-200 bg-slate-50/70 p-4 dark:border-slate-800 dark:bg-slate-800/40">
          <p class="text-xs font-semibold text-slate-900 dark:text-white">
            2. Docker CLI &amp; IDE Support
          </p>
          <p class="mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-slate-300">
            NanoDock listens on <code class="rounded bg-slate-200/70 px-1 py-0.5 font-mono text-[11px] dark:bg-slate-700">\\.\pipe\docker_engine</code>, so standard <code class="rounded bg-slate-200/70 px-1 py-0.5 font-mono text-[11px] dark:bg-slate-700">docker</code>, <code class="rounded bg-slate-200/70 px-1 py-0.5 font-mono text-[11px] dark:bg-slate-700">docker compose</code>, and VS Code Dev Containers work out of the box—or use <code class="rounded bg-slate-200/70 px-1 py-0.5 font-mono text-[11px] dark:bg-slate-700">nanodock &lt;cmd&gt;</code>.
          </p>
        </div>

        <div class="rounded-xl border border-slate-200 bg-slate-50/70 p-4 dark:border-slate-800 dark:bg-slate-800/40">
          <p class="text-xs font-semibold text-slate-900 dark:text-white">
            3. Windows Bind Mounts
          </p>
          <p class="mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-slate-300">
            You can pass Windows paths like <code class="rounded bg-slate-200/70 px-1 py-0.5 font-mono text-[11px] dark:bg-slate-700">C:\Users\dev\app</code> when creating containers. NanoDock automatically translates them to <code class="rounded bg-slate-200/70 px-1 py-0.5 font-mono text-[11px] dark:bg-slate-700">/mnt/c/...</code> inside WSL2.
          </p>
        </div>

        <div class="rounded-xl border border-slate-200 bg-slate-50/70 p-4 dark:border-slate-800 dark:bg-slate-800/40">
          <p class="text-xs font-semibold text-slate-900 dark:text-white">
            4. Disk Reclamation (fstrim)
          </p>
          <p class="mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-slate-300">
            Deleting images or containers frees space inside Linux, but Windows NTFS only reclaims it after <strong>fstrim</strong> runs. Enable <strong>Auto-trim</strong> in Settings or trigger it anytime on the <strong>Storage &amp; Trim</strong> tab.
          </p>
        </div>
      </div>

      <div class="mt-5 flex items-center justify-between border-t border-slate-200 pt-4 dark:border-slate-800">
        <p class="text-[11px] text-slate-400">
          Tip: Hover or click any <span class="font-semibold text-slate-500 dark:text-slate-300">(?)</span> icon across the app for context-specific help.
        </p>
        <button
          type="button"
          onclick={onClose}
          class="rounded-lg bg-sky-600 px-4 py-1.5 text-xs font-semibold text-white transition hover:bg-sky-500"
        >
          Got it
        </button>
      </div>
    </div>
  </div>
{/if}
