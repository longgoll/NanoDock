<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    title,
    text,
    position = "top",
    align = "center",
    label = "More info",
  }: {
    title?: string;
    text: string;
    position?: "top" | "bottom";
    align?: "center" | "left" | "right";
    label?: string;
  } = $props();

  let pinned = $state(false);
  let hovered = $state(false);
  let containerEl = $state<HTMLSpanElement | null>(null);

  const visible = $derived(pinned || hovered);

  const posClass = $derived(
    position === "top" ? "bottom-full mb-2" : "top-full mt-2",
  );

  const alignClass = $derived(
    align === "left"
      ? "left-0"
      : align === "right"
        ? "right-0"
        : "left-1/2 -translate-x-1/2",
  );

  function handleWindowClick(e: MouseEvent) {
    if (pinned && containerEl && !containerEl.contains(e.target as Node)) {
      pinned = false;
    }
  }

  function handleWindowKeydown(e: KeyboardEvent) {
    if (pinned && e.key === "Escape") {
      pinned = false;
    }
  }
</script>

<svelte:window onclick={handleWindowClick} onkeydown={handleWindowKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<span
  bind:this={containerEl}
  class="relative inline-flex items-center align-middle"
  onmouseenter={() => (hovered = true)}
  onmouseleave={() => (hovered = false)}
>
  <button
    type="button"
    onclick={() => (pinned = !pinned)}
    onfocus={() => (hovered = true)}
    onblur={() => (hovered = false)}
    aria-label={label}
    aria-expanded={visible}
    class={`inline-flex h-4 w-4 items-center justify-center rounded-full transition focus:outline-none focus:ring-2 focus:ring-sky-500/40 ${
      pinned
        ? "bg-sky-100 text-sky-700 dark:bg-sky-500/20 dark:text-sky-300"
        : "text-slate-400 hover:bg-slate-100 hover:text-slate-600 dark:text-slate-500 dark:hover:bg-slate-800 dark:hover:text-slate-300"
    }`}
  >
    <Icon name="help" class="h-3.5 w-3.5" />
  </button>

  {#if visible}
    <div
      role="tooltip"
      class={`absolute z-50 w-64 rounded-xl border border-slate-200 bg-white p-3 text-left shadow-xl dark:border-slate-700 dark:bg-slate-900 ${posClass} ${alignClass}`}
    >
      {#if title}
        <p class="mb-1 text-xs font-semibold text-slate-900 dark:text-white">
          {title}
        </p>
      {/if}
      <p class="text-[11px] leading-relaxed text-slate-600 dark:text-slate-300">
        {text}
      </p>
    </div>
  {/if}
</span>
