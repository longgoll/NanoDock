<script lang="ts">
  import { onMount } from "svelte";
  import ContainersView from "./features/containers/ContainersView.svelte";
  import DashboardView from "./features/dashboard/DashboardView.svelte";
  import ImagesView from "./features/images/ImagesView.svelte";
  import SettingsView from "./features/settings/SettingsView.svelte";
  import StorageView from "./features/storage/StorageView.svelte";
  import VolumesView from "./features/volumes/VolumesView.svelte";
  import { engineStore } from "./lib/stores/engine.svelte";

  type Tab = "dashboard" | "containers" | "images" | "volumes" | "storage" | "settings";
  let activeTab = $state<Tab>("dashboard");

  const navItems: { id: Tab; label: string }[] = [
    { id: "dashboard", label: "Dashboard" },
    { id: "containers", label: "Containers" },
    { id: "images", label: "Images" },
    { id: "volumes", label: "Volumes" },
    { id: "storage", label: "Storage & Trim" },
    { id: "settings", label: "Settings" },
  ];

  onMount(() => {
    engineStore.initTheme();
    engineStore.refresh();
  });
</script>

<div class="flex h-screen bg-slate-50 text-slate-900 transition-colors dark:bg-slate-950 dark:text-slate-100">
  <aside class="flex w-60 flex-col justify-between border-r border-slate-200 bg-white p-4 dark:border-slate-800 dark:bg-slate-900/60">
    <div>
      <div class="mb-6 flex items-center justify-between px-2">
        <div class="flex items-center gap-2.5">
          <div class="flex h-7 w-7 items-center justify-center rounded-lg bg-sky-600 text-white shadow-sm">
            <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
              <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
              <line x1="12" y1="22.08" x2="12" y2="12" />
            </svg>
          </div>
          <div>
            <span class="block text-sm font-bold tracking-tight text-slate-900 dark:text-white">NanoDock</span>
            <span class="block text-[10px] font-medium text-slate-400">WSL2 Engine</span>
          </div>
        </div>
      </div>

      <nav class="space-y-1 text-sm">
        {#each navItems as item}
          <button
            onclick={() => (activeTab = item.id)}
            class={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition ${
              activeTab === item.id
                ? "bg-sky-50 font-semibold text-sky-700 dark:bg-sky-500/15 dark:text-sky-400"
                : "text-slate-600 hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800/70 dark:hover:text-slate-200"
            }`}
          >
            {#if item.id === "dashboard"}
              <svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="3" y="3" width="7" height="9" rx="1" />
                <rect x="14" y="3" width="7" height="5" rx="1" />
                <rect x="14" y="12" width="7" height="9" rx="1" />
                <rect x="3" y="16" width="7" height="5" rx="1" />
              </svg>
            {:else if item.id === "containers"}
              <svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z" />
                <path d="m3.3 7 8.7 5 8.7-5" />
                <path d="M12 22V12" />
              </svg>
            {:else if item.id === "images"}
              <svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polygon points="12 2 2 7 12 12 22 7 12 2" />
                <polyline points="2 17 12 22 22 17" />
                <polyline points="2 12 12 17 22 12" />
              </svg>
            {:else if item.id === "volumes"}
              <svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <ellipse cx="12" cy="5" rx="9" ry="3" />
                <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5" />
                <path d="M3 12c0 1.66 4 3 9 3s9-1.34 9-3" />
              </svg>
            {:else if item.id === "storage"}
              <svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="2" y="4" width="20" height="16" rx="2" />
                <path d="M6 12h.01" />
                <path d="M10 12h.01" />
                <path d="M14 12h4" />
              </svg>
            {:else}
              <svg class="h-4 w-4 shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="3" />
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9c.26.604.852.997 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1Z" />
              </svg>
            {/if}
            <span>{item.label}</span>
          </button>
        {/each}
      </nav>
    </div>

    <div class="border-t border-slate-200 pt-3 dark:border-slate-800">
      <button
        onclick={() => engineStore.toggleTheme()}
        class="flex w-full items-center justify-between rounded-lg px-3 py-2 text-xs font-medium text-slate-600 transition hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-slate-800"
      >
        <span>{engineStore.theme === "light" ? "Light Mode" : "Dark Mode"}</span>
        {#if engineStore.theme === "light"}
          <svg class="h-4 w-4 text-amber-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="4" />
            <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
          </svg>
        {:else}
          <svg class="h-4 w-4 text-sky-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" />
          </svg>
        {/if}
      </button>
    </div>
  </aside>

  <main class="flex-1 overflow-y-auto p-7">
    {#if engineStore.error}
      <div class="mb-4 flex items-center justify-between rounded-xl border border-rose-200 bg-rose-50 px-4 py-2.5 text-xs text-rose-700 dark:border-rose-500/30 dark:bg-rose-500/10 dark:text-rose-300">
        <span>{engineStore.error}</span>
        <button onclick={() => (engineStore.error = null)} class="ml-3 font-semibold hover:underline">Dismiss</button>
      </div>
    {/if}

    {#if engineStore.notice}
      <div class="mb-4 flex items-center justify-between rounded-xl border border-emerald-200 bg-emerald-50 px-4 py-2.5 text-xs text-emerald-700 dark:border-emerald-500/30 dark:bg-emerald-500/10 dark:text-emerald-300">
        <span>{engineStore.notice}</span>
        <button onclick={() => (engineStore.notice = null)} class="ml-3 font-semibold hover:underline">Dismiss</button>
      </div>
    {/if}

    {#if activeTab === "dashboard"}
      <DashboardView />
    {:else if activeTab === "containers"}
      <ContainersView />
    {:else if activeTab === "images"}
      <ImagesView />
    {:else if activeTab === "volumes"}
      <VolumesView />
    {:else if activeTab === "storage"}
      <StorageView />
    {:else}
      <SettingsView />
    {/if}
  </main>
</div>
