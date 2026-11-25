<script lang="ts">
  import { i18n } from "../i18n/index.svelte";
  import { engineStore } from "../stores/engine.svelte";
  import Icon from "./Icon.svelte";

  let {
    open = false,
    initialImage = "",
    allowImageEdit = false,
    onClose,
  }: {
    open: boolean;
    initialImage?: string;
    allowImageEdit?: boolean;
    onClose: () => void;
  } = $props();

  let imageRef = $state("");
  let name = $state("");
  let ports = $state("");
  let env = $state("");
  let firstInput = $state<HTMLInputElement | null>(null);

  const localTags = $derived(
    engineStore.images.flatMap((img) =>
      img.repo_tags.filter((t) => t && t !== "<none>:<none>"),
    ),
  );

  $effect(() => {
    if (open) {
      imageRef = initialImage;
      name = "";
      ports = "";
      env = "";
      queueMicrotask(() => firstInput?.focus());
    }
  });

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    const img = imageRef.trim();
    if (!img) return;
    const parsedPorts = ports
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    const parsedEnv = env
      .split("\n")
      .map((s) => s.trim())
      .filter(Boolean);

    const ok = await engineStore.runContainer({
      image: img,
      name: name.trim() || null,
      ports: parsedPorts,
      env: parsedEnv,
    });
    if (ok) {
      onClose();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (open && e.key === "Escape" && !engineStore.busy) {
      onClose();
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget && !engineStore.busy) {
      onClose();
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
    aria-label="Run New Container"
  >
    <form
      onsubmit={handleSubmit}
      class="w-full max-w-md space-y-4 rounded-2xl border border-slate-200 bg-white p-6 shadow-2xl dark:border-slate-800 dark:bg-slate-900"
    >
      <div>
        <h3 class="text-base font-semibold text-slate-900 dark:text-white">{i18n.t.modals.runTitle}</h3>
        {#if !allowImageEdit}
          <p class="mt-0.5 font-mono text-xs text-sky-600 dark:text-sky-400">{imageRef}</p>
        {/if}
      </div>

      {#if allowImageEdit}
        <div>
          <label for="run-image" class="block text-xs font-medium text-slate-700 dark:text-slate-300">
            Image
          </label>
          <input
            id="run-image"
            bind:this={firstInput}
            type="text"
            required
            list="nanodock-local-images"
            bind:value={imageRef}
            placeholder="e.g. nginx:alpine"
            class="mt-1 w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 font-mono text-xs text-slate-900 focus:border-sky-500 focus:bg-white focus:outline-none dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100"
          />
          <datalist id="nanodock-local-images">
            {#each localTags as tag}
              <option value={tag}></option>
            {/each}
          </datalist>
        </div>
      {/if}

      <div>
        <label for="run-name" class="block text-xs font-medium text-slate-700 dark:text-slate-300">
          {i18n.t.modals.runNameLabel}
        </label>
        {#if allowImageEdit}
          <input
            id="run-name"
            type="text"
            bind:value={name}
            placeholder={i18n.t.modals.runNamePlaceholder}
            class="mt-1 w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 text-sm text-slate-900 focus:border-sky-500 focus:bg-white focus:outline-none dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100"
          />
        {:else}
          <input
            id="run-name"
            bind:this={firstInput}
            type="text"
            bind:value={name}
            placeholder={i18n.t.modals.runNamePlaceholder}
            class="mt-1 w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 text-sm text-slate-900 focus:border-sky-500 focus:bg-white focus:outline-none dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100"
          />
        {/if}
      </div>

      <div>
        <label for="run-ports" class="block text-xs font-medium text-slate-700 dark:text-slate-300">
          {i18n.t.modals.runPortsLabel}
        </label>
        <input
          id="run-ports"
          type="text"
          bind:value={ports}
          placeholder="e.g. 8080:80, 8443:443"
          class="mt-1 w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 font-mono text-xs text-slate-900 focus:border-sky-500 focus:bg-white focus:outline-none dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100"
        />
      </div>

      <div>
        <label for="run-env" class="block text-xs font-medium text-slate-700 dark:text-slate-300">
          {i18n.t.modals.runEnvLabel}
        </label>
        <textarea
          id="run-env"
          rows="3"
          bind:value={env}
          placeholder="POSTGRES_PASSWORD=secret"
          class="mt-1 w-full rounded-lg border border-slate-200 bg-slate-50 px-3 py-1.5 font-mono text-xs text-slate-900 focus:border-sky-500 focus:bg-white focus:outline-none dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100"
        ></textarea>
      </div>

      <div class="flex justify-end gap-2 pt-2">
        <button
          type="button"
          onclick={onClose}
          disabled={engineStore.busy}
          class="rounded-lg border border-slate-200 bg-white px-4 py-2 text-xs font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200"
        >
          {i18n.t.common.cancel}
        </button>
        <button
          type="submit"
          disabled={engineStore.busy || !imageRef.trim()}
          class="inline-flex items-center gap-1.5 rounded-lg bg-emerald-600 px-4 py-2 text-xs font-medium text-white transition hover:bg-emerald-500 disabled:opacity-50"
        >
          {#if engineStore.isPending("container:run")}
            <Icon name="spinner" class="h-3.5 w-3.5 animate-spin" />
          {/if}
          {engineStore.isPending("container:run") ? i18n.t.modals.runStarting : i18n.t.containers.runContainer}
        </button>
      </div>
    </form>
  </div>
{/if}
