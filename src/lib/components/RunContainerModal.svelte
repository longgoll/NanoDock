<script lang="ts">
  import { i18n } from "../i18n/index.svelte";
  import { engineStore } from "../stores/engine.svelte";
  import { trapModalTab } from "../utils/focusTrap";
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
  let dialogEl = $state<HTMLFormElement | null>(null);

  const isSubmitting = $derived(engineStore.isPending("container:run"));

  const localTags = $derived(
    engineStore.images.flatMap((img) =>
      img.repo_tags.filter((t) => t && t !== "<none>:<none>"),
    ),
  );

  const quickTemplates = [
    { label: "Postgres 16", image: "postgres:16-alpine", name: "pg-dev", ports: "5432:5432", env: "POSTGRES_PASSWORD=postgres" },
    { label: "Redis 7", image: "redis:7-alpine", name: "redis-dev", ports: "6379:6379", env: "" },
    { label: "MySQL 8", image: "mysql:8.4", name: "mysql-dev", ports: "3306:3306", env: "MYSQL_ROOT_PASSWORD=root" },
    { label: "Nginx", image: "nginx:alpine", name: "nginx-dev", ports: "8080:80", env: "" },
    { label: "Mailpit", image: "axllent/mailpit:latest", name: "mailpit", ports: "8025:8025, 1025:1025", env: "" },
  ];

  function applyTemplate(tpl: (typeof quickTemplates)[number]) {
    imageRef = tpl.image;
    name = tpl.name;
    ports = tpl.ports;
    env = tpl.env;
  }

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
    if (!open) return;
    if (e.key === "Escape" && !isSubmitting) {
      onClose();
      return;
    }
    trapModalTab(e, dialogEl);
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget && !isSubmitting) {
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
      bind:this={dialogEl}
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
          <p class="mb-1.5 text-[11px] font-medium uppercase tracking-wider text-slate-400">
            {i18n.t.extra.templatesTitle}
          </p>
          <div class="mb-3 flex flex-wrap gap-1.5">
            {#each quickTemplates as tpl}
              <button
                type="button"
                onclick={() => applyTemplate(tpl)}
                class="rounded-md border border-slate-200 bg-slate-50 px-2 py-1 text-[11px] font-medium text-slate-700 transition hover:border-sky-400 hover:text-sky-600 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300 dark:hover:border-sky-500/50 dark:hover:text-sky-300"
              >
                {tpl.label}
              </button>
            {/each}
          </div>
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
          disabled={isSubmitting}
          class="rounded-lg border border-slate-200 bg-white px-4 py-2 text-xs font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-200"
        >
          {i18n.t.common.cancel}
        </button>
        <button
          type="submit"
          disabled={isSubmitting || engineStore.engineBusy || !imageRef.trim()}
          class="inline-flex items-center gap-1.5 rounded-lg bg-emerald-600 px-4 py-2 text-xs font-medium text-white transition hover:bg-emerald-500 disabled:opacity-50"
        >
          {#if isSubmitting}
            <Icon name="spinner" class="h-3.5 w-3.5 animate-spin" />
          {/if}
          {isSubmitting ? i18n.t.modals.runStarting : i18n.t.containers.runContainer}
        </button>
      </div>
    </form>
  </div>
{/if}
