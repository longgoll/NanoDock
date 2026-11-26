<script lang="ts">
  import type { ImageLayer } from "../../lib/api/daemon";
  import Icon from "../../lib/components/Icon.svelte";
  import { i18n } from "../../lib/i18n/index.svelte";
  import { formatBytes } from "../../lib/utils/format";

  let {
    open,
    imageName,
    layers,
    loading,
    onClose,
  }: {
    open: boolean;
    imageName: string;
    layers: ImageLayer[];
    loading: boolean;
    onClose: () => void;
  } = $props();

  const maxLayerSize = $derived(
    Math.max(1, ...layers.map((l) => Math.max(0, l.size))),
  );

  function handleKeydown(e: KeyboardEvent) {
    if (open && e.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
  <div
    onclick={(e) => {
      if (e.target === e.currentTarget) onClose();
    }}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    aria-label={i18n.t.extra.layersTitle}
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/60 p-4 backdrop-blur-xs"
  >
    <div class="flex max-h-[85vh] w-full max-w-3xl flex-col rounded-2xl border border-slate-200 bg-white shadow-2xl dark:border-slate-800 dark:bg-slate-900">
      <div class="flex items-center justify-between border-b border-slate-200 px-5 py-3.5 dark:border-slate-800">
        <div>
          <h3 class="text-sm font-semibold text-slate-900 dark:text-slate-100">{i18n.t.extra.layersTitle}</h3>
          <p class="font-mono text-xs text-slate-500 dark:text-slate-400">{imageName}</p>
        </div>
        <button
          type="button"
          onclick={onClose}
          aria-label={i18n.t.common.cancel}
          class="rounded-lg p-1.5 text-slate-400 hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-slate-800 dark:hover:text-slate-100"
        >
          <Icon name="close" class="h-4 w-4" />
        </button>
      </div>

      <div class="flex-1 overflow-y-auto p-5">
        {#if loading}
          <p class="text-xs text-slate-500 dark:text-slate-400">{i18n.t.common.loading}</p>
        {:else if layers.length === 0}
          <p class="text-xs text-slate-500">No layer history available.</p>
        {:else}
          <div class="space-y-2.5">
            {#each layers as layer, idx}
              {@const pct = Math.round((Math.max(0, layer.size) / maxLayerSize) * 100)}
              <div class="rounded-lg border border-slate-200 bg-slate-50 p-3 dark:border-slate-800 dark:bg-slate-950/70">
                <div class="flex items-center justify-between gap-2 text-xs">
                  <span class="font-mono text-sky-600 dark:text-sky-400">#{idx + 1} · {layer.id}</span>
                  <span class="font-mono text-slate-800 dark:text-slate-200">{formatBytes(Math.max(0, layer.size))}</span>
                </div>
                <div class="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-slate-200 dark:bg-slate-800">
                  <div class="h-full rounded-full bg-sky-500" style="width: {pct}%"></div>
                </div>
                <p class="mt-2 break-all font-mono text-[11px] text-slate-600 dark:text-slate-300">
                  {layer.created_by || "(empty)"}
                </p>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}
