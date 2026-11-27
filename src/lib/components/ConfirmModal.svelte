<script lang="ts">
  import { i18n } from "../i18n/index.svelte";
  import { trapModalTab } from "../utils/focusTrap";
  import Icon from "./Icon.svelte";

  let {
    open = false,
    title = "Confirm Action",
    message = "",
    confirmLabel = "Confirm",
    danger = true,
    busy = false,
    onConfirm,
    onCancel,
  }: {
    open: boolean;
    title: string;
    message: string;
    confirmLabel?: string;
    danger?: boolean;
    busy?: boolean;
    onConfirm: () => void;
    onCancel: () => void;
  } = $props();

  let dialogEl = $state<HTMLDivElement | null>(null);
  let cancelBtnEl = $state<HTMLButtonElement | null>(null);

  $effect(() => {
    if (open) {
      queueMicrotask(() => {
        cancelBtnEl?.focus();
      });
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === "Escape" && !busy) {
      onCancel();
      return;
    }
    trapModalTab(e, dialogEl);
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget && !busy) {
      onCancel();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
  <div
    onclick={handleBackdropClick}
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/60 p-4 backdrop-blur-xs"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    aria-label={title}
  >
    <div
      bind:this={dialogEl}
      class="w-full max-w-md rounded-2xl border border-slate-200 bg-white p-6 shadow-2xl dark:border-slate-800 dark:bg-slate-900"
    >
      <div class="flex items-start gap-3.5">
        <div
          class={`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl ${
            danger
              ? "bg-rose-50 text-rose-600 dark:bg-rose-500/15 dark:text-rose-400"
              : "bg-amber-50 text-amber-600 dark:bg-amber-500/15 dark:text-amber-400"
          }`}
        >
          <Icon name="warning" class="h-5 w-5" />
        </div>
        <div class="flex-1">
          <h3 class="text-base font-semibold text-slate-900 dark:text-white">{title}</h3>
          <p class="mt-1.5 text-sm leading-relaxed text-slate-600 dark:text-slate-300">{message}</p>
        </div>
      </div>

      <div class="mt-6 flex justify-end gap-2.5">
        <button
          bind:this={cancelBtnEl}
          type="button"
          onclick={onCancel}
          disabled={busy}
          class="rounded-lg border border-slate-200 bg-white px-4 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200 dark:hover:bg-slate-700"
        >
          {i18n.t.common.cancel}
        </button>
        <button
          type="button"
          onclick={onConfirm}
          disabled={busy}
          class={`inline-flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-medium text-white shadow-xs transition disabled:opacity-50 ${
            danger
              ? "bg-rose-600 hover:bg-rose-500"
              : "bg-amber-600 hover:bg-amber-500"
          }`}
        >
          {#if busy}
            <Icon name="spinner" class="h-3.5 w-3.5 animate-spin" />
          {/if}
          {busy ? i18n.t.common.working : confirmLabel}
        </button>
      </div>
    </div>
  </div>
{/if}
