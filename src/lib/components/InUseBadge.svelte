<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    inUse,
    usedBy = [],
    verb = "Used by",
  }: {
    inUse: boolean;
    usedBy?: string[];
    verb?: string;
  } = $props();
</script>

{#if inUse}
  <span
    class="inline-flex items-center gap-1 rounded-full border border-emerald-200 bg-emerald-50 px-2 py-0.5 text-[11px] font-medium text-emerald-700 dark:border-emerald-500/30 dark:bg-emerald-500/15 dark:text-emerald-300"
    title={`${verb}: ${usedBy.join(", ")}`}
  >
    <Icon name="lock" class="h-3 w-3" />
    In Use
  </span>
  {#if usedBy.length > 0}
    <div
      class="mt-0.5 max-w-[180px] truncate text-[11px] text-slate-400"
      title={usedBy.join(", ")}
    >
      {usedBy.join(", ")}
    </div>
  {/if}
{:else}
  <span
    class="inline-flex rounded-full border border-slate-200 bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-400"
  >
    Unused
  </span>
{/if}
