<script lang="ts">
  import { i18n } from "../i18n/index.svelte";
  import { engineStore } from "../stores/engine.svelte";
  import Icon from "./Icon.svelte";

  let { resourceLabel = "resources" }: { resourceLabel?: string } = $props();
</script>

<div class="rounded-2xl border border-dashed border-slate-300 bg-white p-10 text-center dark:border-slate-800 dark:bg-slate-900/40">
  <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-2xl bg-slate-100 text-slate-500 dark:bg-slate-800 dark:text-slate-400">
    <Icon name="moon" class="h-6 w-6" />
  </div>
  <h3 class="mt-4 text-base font-semibold text-slate-900 dark:text-white">
    {i18n.t.sleepingCard.title}
  </h3>
  <p class="mx-auto mt-1.5 max-w-md text-xs leading-relaxed text-slate-500 dark:text-slate-400">
    {i18n.t.sleepingCard.description(resourceLabel)}
  </p>
  <button
    type="button"
    onclick={() => engineStore.start()}
    disabled={engineStore.busy}
    class="mt-5 inline-flex items-center gap-2 rounded-lg bg-sky-600 px-4 py-2 text-sm font-medium text-white shadow-xs transition hover:bg-sky-500 disabled:opacity-50"
  >
    <Icon
      name={engineStore.isPending("engine:start") ? "spinner" : "play"}
      class={`h-4 w-4 ${engineStore.isPending("engine:start") ? "animate-spin" : ""}`}
    />
    {engineStore.isPending("engine:start") ? i18n.t.sleepingCard.wakingUp : i18n.t.sleepingCard.wakeUp}
  </button>
</div>
