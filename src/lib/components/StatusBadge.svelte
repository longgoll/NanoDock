<script lang="ts">
  import type { EngineState } from "../api/daemon";
  import { i18n } from "../i18n/index.svelte";

  let { state }: { state: EngineState } = $props();

  const styles: Record<EngineState, string> = {
    running:
      "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-500/30 dark:bg-emerald-500/15 dark:text-emerald-400",
    stopped:
      "border-slate-200 bg-slate-100 text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300",
    starting:
      "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/15 dark:text-amber-400",
    stopping:
      "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-500/30 dark:bg-amber-500/15 dark:text-amber-400",
    error:
      "border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-500/30 dark:bg-rose-500/15 dark:text-rose-400",
  };

  const label = $derived(i18n.t.status[state]);
</script>

<span
  class={`inline-flex items-center gap-2 rounded-full border px-3 py-1 text-xs font-medium ${styles[state]}`}
>
  <span class="h-2 w-2 rounded-full bg-current"></span>
  {label}
</span>
