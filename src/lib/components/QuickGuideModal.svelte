<script lang="ts">
  import { i18n } from "../i18n/index.svelte";
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
              {i18n.t.quickGuide.title}
            </h2>
            <p class="text-xs text-slate-500 dark:text-slate-400">
              {i18n.t.quickGuide.subtitle}
            </p>
          </div>
        </div>
        <button
          type="button"
          onclick={onClose}
          aria-label={i18n.t.quickGuide.closeLabel}
          class="rounded-lg p-1.5 text-slate-400 transition hover:bg-slate-100 hover:text-slate-600 dark:hover:bg-slate-800 dark:hover:text-slate-200"
        >
          <Icon name="close" class="h-4 w-4" />
        </button>
      </div>

      <div class="mt-5 grid grid-cols-1 gap-3.5 sm:grid-cols-2">
        <div class="rounded-xl border border-slate-200 bg-slate-50/70 p-4 dark:border-slate-800 dark:bg-slate-800/40">
          <p class="text-xs font-semibold text-slate-900 dark:text-white">
            {i18n.t.quickGuide.card1Title}
          </p>
          <p class="mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-slate-300">
            {i18n.t.quickGuide.card1Body}
          </p>
        </div>

        <div class="rounded-xl border border-slate-200 bg-slate-50/70 p-4 dark:border-slate-800 dark:bg-slate-800/40">
          <p class="text-xs font-semibold text-slate-900 dark:text-white">
            {i18n.t.quickGuide.card2Title}
          </p>
          <p class="mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-slate-300">
            {i18n.t.quickGuide.card2Body}
          </p>
        </div>

        <div class="rounded-xl border border-slate-200 bg-slate-50/70 p-4 dark:border-slate-800 dark:bg-slate-800/40">
          <p class="text-xs font-semibold text-slate-900 dark:text-white">
            {i18n.t.quickGuide.card3Title}
          </p>
          <p class="mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-slate-300">
            {i18n.t.quickGuide.card3Body}
          </p>
        </div>

        <div class="rounded-xl border border-slate-200 bg-slate-50/70 p-4 dark:border-slate-800 dark:bg-slate-800/40">
          <p class="text-xs font-semibold text-slate-900 dark:text-white">
            {i18n.t.quickGuide.card4Title}
          </p>
          <p class="mt-1.5 text-xs leading-relaxed text-slate-600 dark:text-slate-300">
            {i18n.t.quickGuide.card4Body}
          </p>
        </div>
      </div>

      <div class="mt-5 flex items-center justify-between border-t border-slate-200 pt-4 dark:border-slate-800">
        <p class="text-[11px] text-slate-400">
          {i18n.t.quickGuide.tip}
        </p>
        <button
          type="button"
          onclick={onClose}
          class="rounded-lg bg-sky-600 px-4 py-1.5 text-xs font-semibold text-white transition hover:bg-sky-500"
        >
          {i18n.t.quickGuide.gotIt}
        </button>
      </div>
    </div>
  </div>
{/if}
