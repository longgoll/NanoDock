<script lang="ts">
  import { onMount } from "svelte";
  import { daemonApi, type ContainerFileEntry } from "../../lib/api/daemon";
  import Icon from "../../lib/components/Icon.svelte";
  import { i18n } from "../../lib/i18n/index.svelte";
  import { formatBytes } from "../../lib/utils/format";

  let { containerId }: { containerId: string } = $props();

  let currentPath = $state("/");
  let entries = $state<ContainerFileEntry[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  let previewPath = $state<string | null>(null);
  let previewContent = $state("");
  let previewTruncated = $state(false);
  let previewLoading = $state(false);
  let copiedKey = $state<"path" | "content" | null>(null);

  const breadcrumbs = $derived.by(() => {
    const parts = currentPath.split("/").filter(Boolean);
    return parts.map((label, idx) => ({
      label,
      path: `/${parts.slice(0, idx + 1).join("/")}`,
    }));
  });

  async function copyText(key: "path" | "content", text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedKey = key;
      setTimeout(() => {
        if (copiedKey === key) copiedKey = null;
      }, 1400);
    } catch {
      // ignore
    }
  }

  async function loadDir(path: string) {
    loading = true;
    error = null;
    try {
      const resp = await daemonApi.listContainerFiles(containerId, path);
      if (resp.status === "container_files") {
        currentPath = path;
        entries = resp.data;
      } else if (resp.status === "error") {
        error = resp.data.message;
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function openEntry(entry: ContainerFileEntry) {
    if (entry.is_dir) {
      previewPath = null;
      await loadDir(entry.path);
      return;
    }
    previewLoading = true;
    previewPath = entry.path;
    error = null;
    try {
      const resp = await daemonApi.readContainerFile(containerId, entry.path);
      if (resp.status === "container_file_content") {
        previewContent = resp.data.content;
        previewTruncated = resp.data.truncated;
      } else if (resp.status === "error") {
        error = resp.data.message;
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      previewLoading = false;
    }
  }

  function goUp() {
    if (currentPath === "/") return;
    const parts = currentPath.split("/").filter(Boolean);
    parts.pop();
    const next = parts.length === 0 ? "/" : `/${parts.join("/")}`;
    void loadDir(next);
  }

  onMount(() => {
    void loadDir("/");
  });
</script>

<div class="flex flex-col gap-3">
  <div class="flex flex-wrap items-center gap-2 rounded-xl border border-slate-200 bg-white px-3 py-2 shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
    <button
      type="button"
      onclick={goUp}
      disabled={currentPath === "/" || loading}
      class="rounded-md border border-slate-200 px-2 py-1 text-xs text-slate-700 hover:bg-slate-100 disabled:opacity-40 dark:border-slate-700 dark:text-slate-300 dark:hover:bg-slate-800"
    >
      ↑ {i18n.t.detail.filesUp}
    </button>
    <nav aria-label="Container path" class="flex min-w-0 flex-1 flex-wrap items-center gap-1 font-mono text-xs">
      <button
        type="button"
        onclick={() => {
          previewPath = null;
          void loadDir("/");
        }}
        disabled={loading}
        class={`rounded px-1.5 py-0.5 transition hover:bg-slate-100 dark:hover:bg-slate-800 ${
          currentPath === "/"
            ? "font-semibold text-sky-600 dark:text-sky-400"
            : "text-slate-600 dark:text-slate-300"
        }`}
      >
        /
      </button>
      {#each breadcrumbs as crumb, idx (crumb.path)}
        <span class="text-slate-400">/</span>
        <button
          type="button"
          onclick={() => {
            previewPath = null;
            void loadDir(crumb.path);
          }}
          disabled={loading}
          class={`truncate rounded px-1.5 py-0.5 transition hover:bg-slate-100 dark:hover:bg-slate-800 ${
            idx === breadcrumbs.length - 1
              ? "font-semibold text-sky-600 dark:text-sky-400"
              : "text-slate-600 dark:text-slate-300"
          }`}
        >
          {crumb.label}
        </button>
      {/each}
    </nav>
    <button
      type="button"
      onclick={() => loadDir(currentPath)}
      disabled={loading}
      aria-label={i18n.t.common.refresh}
      class="ml-auto rounded-md p-1 text-sky-600 hover:bg-sky-50 dark:text-sky-400 dark:hover:bg-sky-500/15"
    >
      <Icon name="refresh" class={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
    </button>
  </div>

  {#if error}
    <div class="rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-xs text-rose-700 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-300">
      {error}
    </div>
  {/if}

  <div class="grid grid-cols-1 gap-3 lg:grid-cols-2">
    <div class="h-[calc(100vh-20rem)] min-h-[22rem] overflow-y-auto rounded-xl border border-slate-200 bg-white shadow-xs dark:border-slate-800 dark:bg-slate-950/70">
      {#if loading}
        <p class="p-4 text-xs text-slate-500 dark:text-slate-400">{i18n.t.common.loading}</p>
      {:else if entries.length === 0}
        <p class="p-4 text-xs text-slate-500">{i18n.t.detail.filesEmpty}</p>
      {:else}
        <div class="divide-y divide-slate-200 dark:divide-slate-800/70">
          {#each entries as entry}
            <button
              type="button"
              onclick={() => openEntry(entry)}
              class="flex w-full items-center justify-between px-3 py-2 text-left text-xs transition hover:bg-slate-50 dark:hover:bg-slate-800/50"
            >
              <span class="flex items-center gap-2 truncate font-mono {entry.is_dir ? 'text-sky-600 dark:text-sky-400' : 'text-slate-800 dark:text-slate-200'}">
                <Icon name={entry.is_dir ? "folder" : "file"} class="h-3.5 w-3.5 shrink-0" />
                {entry.name}{entry.is_dir ? "/" : ""}
              </span>
              <span class="ml-2 shrink-0 text-[11px] text-slate-500">
                {entry.is_dir ? entry.mode : formatBytes(entry.size)}
              </span>
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="h-[calc(100vh-20rem)] min-h-[22rem] overflow-y-auto rounded-xl border border-slate-200 bg-slate-50 p-3 font-mono text-xs dark:border-slate-800 dark:bg-slate-950">
      {#if !previewPath}
        <p class="text-slate-500">Select a file to preview its contents.</p>
      {:else if previewLoading}
        <p class="text-slate-500 dark:text-slate-400">{i18n.t.common.loading}</p>
      {:else}
        <div class="mb-2 flex flex-wrap items-center justify-between gap-2 border-b border-slate-200 pb-1.5 text-[11px] text-slate-500 dark:border-slate-800 dark:text-slate-400">
          <span class="truncate">{previewPath}</span>
          <div class="flex items-center gap-2">
            {#if previewTruncated}
              <span class="text-amber-600 dark:text-amber-400">{i18n.t.detail.filesPreviewTruncated}</span>
            {/if}
            <button
              type="button"
              onclick={() => previewPath && copyText("path", previewPath)}
              class="rounded border border-slate-200 bg-white px-1.5 py-0.5 text-[10px] font-medium text-slate-600 transition hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300 dark:hover:bg-slate-700"
            >
              {copiedKey === "path" ? i18n.t.common.copied : "Path"}
            </button>
            <button
              type="button"
              onclick={() => copyText("content", previewContent)}
              class="rounded border border-slate-200 bg-white px-1.5 py-0.5 text-[10px] font-medium text-slate-600 transition hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-800 dark:text-slate-300 dark:hover:bg-slate-700"
            >
              {copiedKey === "content" ? i18n.t.common.copied : i18n.t.common.copy}
            </button>
          </div>
        </div>
        <pre class="whitespace-pre-wrap break-all text-slate-800 dark:text-slate-200">{previewContent}</pre>
      {/if}
    </div>
  </div>
</div>
