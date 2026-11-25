<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    text,
    position = "top",
    align = "center",
    maxWidth = "max-w-xs",
    children,
  }: {
    text: string;
    position?: "top" | "bottom";
    align?: "center" | "left" | "right";
    maxWidth?: string;
    children: Snippet;
  } = $props();

  let open = $state(false);
  let triggerEl = $state<HTMLSpanElement | null>(null);
  let coords = $state({ top: 0, left: 0 });

  function updatePosition() {
    if (!triggerEl) return;
    const rect = triggerEl.getBoundingClientRect();
    const top = position === "top" ? rect.top - 6 : rect.bottom + 6;
    const left =
      align === "left"
        ? rect.left
        : align === "right"
          ? rect.right
          : rect.left + rect.width / 2;
    coords = { top, left };
  }

  function show() {
    updatePosition();
    open = true;
  }

  const transformClass = $derived.by(() => {
    const x =
      align === "left"
        ? "0%"
        : align === "right"
          ? "-100%"
          : "-50%";
    const y = position === "top" ? "-100%" : "0%";
    return `transform: translate(${x}, ${y});`;
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<span
  bind:this={triggerEl}
  class="relative inline-flex items-center"
  onmouseenter={show}
  onmouseleave={() => (open = false)}
  onfocusin={show}
  onfocusout={() => (open = false)}
>
  {@render children()}
  {#if open && text}
    <span
      role="tooltip"
      style={`position: fixed; top: ${coords.top}px; left: ${coords.left}px; ${transformClass}`}
      class={`pointer-events-none z-[100] ${maxWidth} w-max rounded-lg border border-slate-700/80 bg-slate-900 px-2.5 py-1.5 text-[11px] font-medium leading-snug text-slate-100 shadow-lg dark:border-slate-700 dark:bg-slate-800 dark:text-slate-100`}
    >
      {text}
    </span>
  {/if}
</span>
