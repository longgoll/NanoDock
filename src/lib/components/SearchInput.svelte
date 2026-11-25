<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    value = $bindable(""),
    placeholder = "Filter...",
    class: className = "w-60",
  }: {
    value: string;
    placeholder?: string;
    class?: string;
  } = $props();
</script>

<div class="relative">
  <Icon
    name="search"
    class="pointer-events-none absolute top-1/2 left-2.5 h-3.5 w-3.5 -translate-y-1/2 text-slate-400"
  />
  <input
    type="text"
    bind:value
    {placeholder}
    aria-label={placeholder}
    class={`${className} rounded-lg border border-slate-200 bg-white py-1.5 pr-7 pl-8 text-xs text-slate-900 placeholder-slate-400 focus:border-sky-500 focus:outline-none dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100`}
  />
  {#if value}
    <button
      type="button"
      onclick={() => (value = "")}
      class="absolute top-1/2 right-2 -translate-y-1/2 text-slate-400 hover:text-slate-600 dark:hover:text-slate-200"
      title="Clear filter"
      aria-label="Clear filter"
    >
      <Icon name="close" class="h-3.5 w-3.5" />
    </button>
  {/if}
</div>
