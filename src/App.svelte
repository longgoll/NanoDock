<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { onMount } from "svelte";
  import ContainersView from "./features/containers/ContainersView.svelte";
  import DashboardView from "./features/dashboard/DashboardView.svelte";
  import ImagesView from "./features/images/ImagesView.svelte";
  import NetworksView from "./features/networks/NetworksView.svelte";
  import SettingsView from "./features/settings/SettingsView.svelte";
  import StorageView from "./features/storage/StorageView.svelte";
  import VolumesView from "./features/volumes/VolumesView.svelte";
  import Icon, { type IconName } from "./lib/components/Icon.svelte";
  import QuickGuideModal from "./lib/components/QuickGuideModal.svelte";
  import { i18n } from "./lib/i18n/index.svelte";
  import { engineStore } from "./lib/stores/engine.svelte";
  import { updaterStore } from "./lib/stores/updater.svelte";

  type Tab = "dashboard" | "containers" | "images" | "volumes" | "networks" | "storage" | "settings";
  let activeTab = $state<Tab>("dashboard");
  let sidebarCollapsed = $state(false);
  let quickGuideOpen = $state(false);
  let appVersion = $state<string | null>(null);

  const navItems = $derived<{ id: Tab; label: string; icon: IconName }[]>([
    { id: "dashboard", label: i18n.t.nav.dashboard, icon: "dashboard" },
    { id: "containers", label: i18n.t.nav.containers, icon: "containers" },
    { id: "images", label: i18n.t.nav.images, icon: "images" },
    { id: "volumes", label: i18n.t.nav.volumes, icon: "volumes" },
    { id: "networks", label: i18n.t.nav.networks, icon: "network" },
    { id: "storage", label: i18n.t.nav.storage, icon: "storage" },
    { id: "settings", label: i18n.t.nav.settings, icon: "settings" },
  ]);

  onMount(() => {
    engineStore.initTheme();
    sidebarCollapsed = localStorage.getItem("nanodock-sidebar-collapsed") === "true";

    void getVersion()
      .then((v) => {
        appVersion = v;
      })
      .catch(() => {
        appVersion = null;
      });

    void engineStore.refreshAll(true);
    void updaterStore.checkForUpdates(true);

    const syncAll = () => {
      if (document.hidden || !engineStore.autoRefresh || engineStore.busy) return;
      void engineStore.refreshAll(true);
    void updaterStore.checkForUpdates(true);
    };

    const timer = setInterval(syncAll, 5000);
    const onFocus = () => syncAll();
    const onVisibilityChange = () => {
      if (!document.hidden) syncAll();
    };

    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  });

  function toggleSidebar() {
    sidebarCollapsed = !sidebarCollapsed;
    localStorage.setItem("nanodock-sidebar-collapsed", String(sidebarCollapsed));
  }

  function navBadge(id: Tab): string | null {
    if (engineStore.state === "stopped") return null;
    if (id === "containers" && engineStore.containers.length > 0) {
      return `${engineStore.runningContainers}/${engineStore.containers.length}`;
    }
    if (id === "images" && engineStore.images.length > 0) {
      return `${engineStore.images.length}`;
    }
    if (id === "volumes" && engineStore.volumes.length > 0) {
      return `${engineStore.volumes.length}`;
    }
    if (id === "networks" && engineStore.networks.length > 0) {
      return `${engineStore.networks.length}`;
    }
    return null;
  }
</script>

<div class="flex h-screen bg-slate-50 text-slate-900 transition-colors dark:bg-slate-950 dark:text-slate-100">
  <aside
    class={`flex shrink-0 flex-col justify-between border-r border-slate-200 bg-white transition-all duration-200 dark:border-slate-800 dark:bg-slate-900/60 ${
      sidebarCollapsed ? "w-16 p-2.5" : "w-60 p-4"
    }`}
  >
    <div>
      <div class={`mb-6 flex items-center ${sidebarCollapsed ? "justify-center" : "justify-between px-2"}`}>
        <div class="flex items-center gap-2.5" title={appVersion ? `NanoDock v${appVersion}` : "NanoDock"}>
          <div class="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-sky-600 text-white shadow-sm">
            <Icon name="logo" class="h-4 w-4" />
          </div>
          {#if !sidebarCollapsed}
            <div>
              <span class="block text-sm font-bold tracking-tight text-slate-900 dark:text-white">NanoDock</span>
              <span class="block text-[10px] font-medium text-slate-400">
                WSL2 Engine{#if appVersion} · v{appVersion}{/if}
              </span>
            </div>
          {/if}
        </div>
        {#if !sidebarCollapsed}
          <button
            type="button"
            onclick={toggleSidebar}
            title={i18n.t.nav.collapseSidebar}
            aria-label={i18n.t.nav.collapseSidebar}
            class="rounded-md p-1 text-slate-400 transition hover:bg-slate-100 hover:text-slate-600 dark:hover:bg-slate-800 dark:hover:text-slate-200"
          >
            <Icon name="chevron-left" class="h-4 w-4" />
          </button>
        {/if}
      </div>

      <nav class="space-y-1 text-sm" aria-label="Main navigation">
        {#each navItems as item}
          {@const badge = navBadge(item.id)}
          <button
            type="button"
            onclick={() => (activeTab = item.id)}
            title={sidebarCollapsed ? item.label : undefined}
            aria-label={item.label}
            class={`flex w-full items-center rounded-lg py-2 transition ${
              sidebarCollapsed ? "justify-center px-2" : "gap-3 px-3 text-left"
            } ${
              activeTab === item.id
                ? "bg-sky-50 font-semibold text-sky-700 dark:bg-sky-500/15 dark:text-sky-400"
                : "text-slate-600 hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800/70 dark:hover:text-slate-200"
            }`}
          >
            <Icon name={item.icon} class="h-4 w-4 shrink-0" />
            {#if !sidebarCollapsed}
              <span class="flex-1 truncate">{item.label}</span>
              {#if badge}
                <span class="rounded-full bg-slate-100 px-2 py-0.5 text-[10px] font-semibold text-slate-600 dark:bg-slate-800 dark:text-slate-300">
                  {badge}
                </span>
              {/if}
            {/if}
          </button>
        {/each}
      </nav>
    </div>

    <div class="space-y-2 border-t border-slate-200 pt-3 dark:border-slate-800">
      {#if sidebarCollapsed}
        <button
          type="button"
          onclick={toggleSidebar}
          title={i18n.t.nav.expandSidebar}
          aria-label={i18n.t.nav.expandSidebar}
          class="flex w-full items-center justify-center rounded-lg p-2 text-slate-400 transition hover:bg-slate-100 hover:text-slate-600 dark:hover:bg-slate-800 dark:hover:text-slate-200"
        >
          <Icon name="chevron-right" class="h-4 w-4" />
        </button>
        <div
          class="flex justify-center py-1"
          title={`${i18n.t.nav.engine}: ${engineStore.state === "running" ? i18n.t.nav.running : engineStore.state === "stopped" ? i18n.t.nav.sleeping : engineStore.state}`}
        >
          <span
            class={`h-2.5 w-2.5 rounded-full ${
              engineStore.state === "running"
                ? "bg-emerald-500"
                : engineStore.state === "error"
                  ? "bg-rose-500"
                  : "bg-slate-400"
            }`}
          ></span>
        </div>
        <button
          type="button"
          onclick={() => (quickGuideOpen = true)}
          title={i18n.t.nav.quickGuide}
          aria-label={i18n.t.nav.quickGuide}
          class="flex w-full items-center justify-center rounded-lg p-2 text-slate-500 transition hover:bg-slate-100 hover:text-sky-600 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-sky-400"
        >
          <Icon name="help" class="h-4 w-4" />
        </button>
        <button
          type="button"
          onclick={() => engineStore.toggleTheme()}
          title={engineStore.theme === "light" ? i18n.t.nav.switchToDark : i18n.t.nav.switchToLight}
          aria-label="Toggle theme"
          class="flex w-full items-center justify-center rounded-lg p-2 text-slate-600 transition hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-slate-800"
        >
          <Icon
            name={engineStore.theme === "light" ? "sun" : "moon"}
            class={`h-4 w-4 ${engineStore.theme === "light" ? "text-amber-500" : "text-sky-400"}`}
          />
        </button>
      {:else}
        <div class="flex items-center justify-between px-3 py-1.5">
          <span class="text-[11px] font-medium text-slate-400">{i18n.t.nav.engine}</span>
          <span
            class={`inline-flex items-center gap-1.5 text-[11px] font-semibold ${
              engineStore.state === "running"
                ? "text-emerald-600 dark:text-emerald-400"
                : engineStore.state === "error"
                  ? "text-rose-600 dark:text-rose-400"
                  : "text-slate-500 dark:text-slate-400"
            }`}
          >
            <span class="h-1.5 w-1.5 rounded-full bg-current"></span>
            {engineStore.state === "running"
              ? i18n.t.nav.running
              : engineStore.state === "stopped"
                ? i18n.t.nav.sleeping
                : engineStore.state}
          </span>
        </div>
        <div class="flex items-center justify-between px-3 py-1">
          <span class="text-[11px] font-medium text-slate-400">{i18n.t.nav.autoSync}</span>
          <button
            type="button"
            onclick={() => engineStore.toggleAutoRefresh()}
            aria-label="Toggle auto-sync every 5 seconds"
            class={`rounded-full px-2 py-0.5 text-[10px] font-semibold transition ${
              engineStore.autoRefresh
                ? "bg-sky-100 text-sky-700 dark:bg-sky-500/20 dark:text-sky-300"
                : "bg-slate-100 text-slate-500 dark:bg-slate-800 dark:text-slate-400"
            }`}
          >
            {engineStore.autoRefresh ? "ON" : "OFF"}
          </button>
        </div>
        <div class="flex items-center gap-1.5">
          <button
            type="button"
            onclick={() => engineStore.toggleTheme()}
            class="flex flex-1 items-center justify-between rounded-lg px-3 py-2 text-xs font-medium text-slate-600 transition hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-slate-800"
          >
            <span>{engineStore.theme === "light" ? i18n.t.nav.lightMode : i18n.t.nav.darkMode}</span>
            <Icon
              name={engineStore.theme === "light" ? "sun" : "moon"}
              class={`h-4 w-4 ${engineStore.theme === "light" ? "text-amber-500" : "text-sky-400"}`}
            />
          </button>
          <button
            type="button"
            onclick={() => (quickGuideOpen = true)}
            title={i18n.t.nav.quickGuide}
            aria-label={i18n.t.nav.quickGuide}
            class="rounded-lg p-2 text-slate-500 transition hover:bg-slate-100 hover:text-sky-600 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-sky-400"
          >
            <Icon name="help" class="h-4 w-4" />
          </button>
        </div>
      {/if}
    </div>
  </aside>

  <main class="relative flex-1 overflow-y-auto p-7">
    {#if engineStore.busy || engineStore.pullingRef || engineStore.loadingResources}
      <div
        aria-hidden="true"
        class="pointer-events-none fixed inset-x-0 top-0 z-50 h-0.5 overflow-hidden bg-sky-500/15"
      >
        <div class="h-full w-1/3 animate-pulse bg-sky-500"></div>
      </div>
    {/if}
    {#if updaterStore.availableVersion && !updaterStore.dismissed}
      <div class="mb-5 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-sky-200 bg-sky-50/80 px-4 py-2.5 text-xs text-sky-900 dark:border-sky-500/30 dark:bg-sky-500/10 dark:text-sky-200">
        <div class="flex items-center gap-2.5">
          <Icon name="info" class="h-4 w-4 shrink-0 text-sky-600 dark:text-sky-400" />
          <span class="font-medium">
            {i18n.t.settings.updateAvailable(updaterStore.availableVersion)}
          </span>
        </div>
        <div class="flex items-center gap-2">
          <button
            type="button"
            onclick={() => void updaterStore.installUpdate()}
            disabled={updaterStore.downloading || updaterStore.installing}
            class="inline-flex items-center gap-1.5 rounded-lg bg-sky-600 px-3 py-1.5 font-medium text-white shadow-xs transition hover:bg-sky-500 disabled:opacity-60"
          >
            {#if updaterStore.downloading || updaterStore.installing}
              <Icon name="spinner" class="h-3.5 w-3.5 animate-spin" />
            {/if}
            {updaterStore.installing
              ? i18n.t.settings.installingUpdate
              : updaterStore.downloading
                ? i18n.t.settings.downloadingUpdate(updaterStore.progress)
                : i18n.t.settings.updateNow}
          </button>
          {#if !updaterStore.downloading && !updaterStore.installing}
            <button
              type="button"
              onclick={() => updaterStore.dismissBanner()}
              class="rounded-lg px-2 py-1 text-sky-700 transition hover:bg-sky-100 dark:text-sky-300 dark:hover:bg-sky-500/20"
            >
              {i18n.t.settings.later}
            </button>
          {/if}
        </div>
      </div>
    {/if}
    {#if activeTab === "dashboard"}
      <DashboardView onNavigate={(tab) => (activeTab = tab)} />
    {:else if activeTab === "containers"}
      <ContainersView />
    {:else if activeTab === "images"}
      <ImagesView />
    {:else if activeTab === "volumes"}
      <VolumesView />
    {:else if activeTab === "networks"}
      <NetworksView />
    {:else if activeTab === "storage"}
      <StorageView />
    {:else}
      <SettingsView />
    {/if}
  </main>

  <!-- Floating Toast Notifications -->
  {#if engineStore.error || engineStore.notice}
    <div class="fixed right-5 bottom-5 z-40 flex max-w-md flex-col gap-2">
      {#if engineStore.error}
        <div
          role="alert"
          class="animate-toast-in flex items-start justify-between gap-3 rounded-xl border border-rose-200 bg-white/95 px-4 py-3 text-xs text-rose-700 shadow-lg backdrop-blur-xs dark:border-rose-500/30 dark:bg-slate-900/95 dark:text-rose-300"
        >
          <div class="flex items-start gap-2">
            <Icon name="warning" class="mt-0.5 h-3.5 w-3.5 shrink-0 text-rose-500" />
            <span class="leading-relaxed">{engineStore.error}</span>
          </div>
          <button
            type="button"
            onclick={() => (engineStore.error = null)}
            aria-label="Dismiss error"
            class="shrink-0 rounded p-0.5 text-rose-500 hover:bg-rose-50 dark:hover:bg-rose-500/15"
          >
            <Icon name="close" class="h-3.5 w-3.5" />
          </button>
        </div>
      {/if}

      {#if engineStore.notice}
        <div
          role="status"
          class="animate-toast-in flex items-start justify-between gap-3 rounded-xl border border-emerald-200 bg-white/95 px-4 py-3 text-xs text-emerald-700 shadow-lg backdrop-blur-xs dark:border-emerald-500/30 dark:bg-slate-900/95 dark:text-emerald-300"
        >
          <div class="flex items-start gap-2">
            <Icon name="check" class="mt-0.5 h-3.5 w-3.5 shrink-0 text-emerald-500" />
            <span class="leading-relaxed">{engineStore.notice}</span>
          </div>
          <button
            type="button"
            onclick={() => engineStore.setNotice(null)}
            aria-label="Dismiss notice"
            class="shrink-0 rounded p-0.5 text-emerald-500 hover:bg-emerald-50 dark:hover:bg-emerald-500/15"
          >
            <Icon name="close" class="h-3.5 w-3.5" />
          </button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<QuickGuideModal open={quickGuideOpen} onClose={() => (quickGuideOpen = false)} />
