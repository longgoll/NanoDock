<script lang="ts">
  import { i18n } from "../i18n/index.svelte";
  import { engineStore } from "../stores/engine.svelte";
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

  const localizedVerb = $derived(
    verb === "Mounted by"
      ? i18n.t.inUseBadge.mountedBy
      : verb === "Used by"
        ? i18n.t.inUseBadge.usedBy
        : verb,
  );
</script>

{#if inUse}
  <span
    class="inline-flex items-center gap-1 rounded-full border border-emerald-200 bg-emerald-50 px-2 py-0.5 text-[11px] font-medium text-emerald-700 dark:border-emerald-500/30 dark:bg-emerald-500/15 dark:text-emerald-300"
    title={`${localizedVerb}: ${usedBy.join(", ")}`}
  >
    <Icon name="lock" class="h-3 w-3" />
    {i18n.t.inUseBadge.inUse}
  </span>
  {#if usedBy.length > 0}
    <div class="mt-1 flex max-w-[200px] flex-wrap gap-1">
      {#each usedBy as name}
        {@const targetId = engineStore.findContainerIdByName(name)}
        {#if targetId}
          <button
            type="button"
            onclick={() => engineStore.openContainerDetail(targetId)}
            title={name}
            class="max-w-[180px] truncate rounded bg-slate-100 px-1.5 py-0.5 text-[11px] font-medium text-sky-700 transition hover:bg-sky-50 hover:underline dark:bg-slate-800 dark:text-sky-400 dark:hover:bg-sky-500/15"
          >
            {name}
          </button>
        {:else}
          <span
            class="max-w-[180px] truncate text-[11px] text-slate-400"
            title={name}
          >
            {name}
          </span>
        {/if}
      {/each}
    </div>
  {/if}
{:else}
  <span
    class="inline-flex rounded-full border border-slate-200 bg-slate-100 px-2 py-0.5 text-[11px] font-medium text-slate-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-400"
  >
    {i18n.t.inUseBadge.unused}
  </span>
{/if}
