<script lang="ts">
  import { i18n } from "../i18n/index.svelte";
  import { engineStore } from "../stores/engine.svelte";
  import Icon from "./Icon.svelte";

  let {
    onRefresh,
    disabled = false,
    size = "sm",
    showNotice = true,
  }: {
    onRefresh: () => Promise<unknown>;
    disabled?: boolean;
    size?: "xs" | "sm" | "md";
    showNotice?: boolean;
  } = $props();

  let spinning = $state(false);
  let justRefreshed = $state(false);
  let resetTimer: ReturnType<typeof setTimeout> | null = null;

  const sizeClass = $derived(
    size === "md"
      ? "px-3.5 py-2 text-sm gap-1.5"
      : size === "xs"
        ? "px-2.5 py-1 text-xs gap-1"
        : "px-3 py-1.5 text-xs gap-1.5",
  );

  const iconClass = $derived(size === "md" ? "h-4 w-4" : "h-3.5 w-3.5");

  async function handleClick() {
    if (spinning || disabled) return;
    if (resetTimer) {
      clearTimeout(resetTimer);
      resetTimer = null;
    }
    justRefreshed = false;
    spinning = true;
    const minSpin = new Promise((r) => setTimeout(r, 260));
    try {
      await Promise.all([onRefresh(), minSpin]);
      if (!engineStore.error) {
        justRefreshed = true;
        if (showNotice) {
          engineStore.setNotice(i18n.t.notices.refreshed);
        }
        resetTimer = setTimeout(() => {
          justRefreshed = false;
        }, 1400);
      }
    } finally {
      spinning = false;
    }
  }
</script>

<button
  type="button"
  onclick={handleClick}
  disabled={disabled || spinning}
  class={`inline-flex items-center rounded-lg border font-medium transition disabled:opacity-50 ${sizeClass} ${
    justRefreshed
      ? "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-500/30 dark:bg-emerald-500/15 dark:text-emerald-300"
      : "border-slate-200 bg-white text-slate-700 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
  }`}
>
  <Icon
    name={justRefreshed ? "check" : "refresh"}
    class={`${iconClass} ${spinning ? "animate-spin text-sky-600 dark:text-sky-400" : ""}`}
  />
  {spinning ? i18n.t.common.refreshing : justRefreshed ? i18n.t.common.refreshed : i18n.t.common.refresh}
</button>
