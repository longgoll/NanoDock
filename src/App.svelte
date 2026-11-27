<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import ContainersView from "./features/containers/ContainersView.svelte";
  import DashboardView from "./features/dashboard/DashboardView.svelte";
  import ImagesView from "./features/images/ImagesView.svelte";
  import NetworksView from "./features/networks/NetworksView.svelte";
  import SettingsView from "./features/settings/SettingsView.svelte";
  import StorageView from "./features/storage/StorageView.svelte";
  import VolumesView from "./features/volumes/VolumesView.svelte";
  import ConfirmModal from "./lib/components/ConfirmModal.svelte";
  import Icon, { type IconName } from "./lib/components/Icon.svelte";
  import QuickGuideModal from "./lib/components/QuickGuideModal.svelte";
  import { i18n } from "./lib/i18n/index.svelte";
  import { engineStore, type AppTab } from "./lib/stores/engine.svelte";
  import { environmentStore } from "./lib/stores/environment.svelte";
  import { updaterStore } from "./lib/stores/updater.svelte";
  import { createEventRefreshScheduler, type DaemonEventPayload } from "./lib/utils/eventScheduler";

  let sidebarCollapsed = $state(false);
  let quickGuideOpen = $state(false);
  let appVersion = $state<string | null>(null);
  let pendingNavTab = $state<AppTab | null>(null);

  const navItems = $derived<{ id: AppTab; label: string; icon: IconName }[]>([
    { id: "dashboard", label: i18n.t.nav.dashboard, icon: "dashboard" },
    { id: "containers", label: i18n.t.nav.containers, icon: "containers" },
    { id: "images", label: i18n.t.nav.images, icon: "images" },
    { id: "volumes", label: i18n.t.nav.volumes, icon: "volumes" },
    { id: "networks", label: i18n.t.nav.networks, icon: "network" },
    { id: "storage", label: i18n.t.nav.storage, icon: "storage" },
    { id: "settings", label: i18n.t.nav.settings, icon: "settings" },
  ]);

  function navigateTo(tab: AppTab) {
    if (tab === engineStore.activeTab) return;
    if (engineStore.activeTab === "settings" && engineStore.settingsDirty) {
      pendingNavTab = tab;
      return;
    }
    engineStore.activeTab = tab;
  }

  function confirmDiscardSettings() {
    const target = pendingNavTab;
    pendingNavTab = null;
    engineStore.settingsDirty = false;
    if (target) {
      engineStore.activeTab = target;
    }
  }

  function handleGlobalKeydown(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return;
    if (e.key >= "1" && e.key <= "7") {
      const target = e.target as HTMLElement | null;
      if (target?.closest(".xterm")) return;
      const idx = Number.parseInt(e.key, 10) - 1;
      const item = navItems[idx];
      if (item) {
        e.preventDefault();
        navigateTo(item.id);
      }
    }
  }

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
    void environmentStore.refresh();

    const syncAll = () => {
      if (document.hidden || !engineStore.autoRefresh || engineStore.busy) return;
      void engineStore.refreshAll(true);
    };

    const timer = setInterval(syncAll, 30000);
    const onFocus = () => syncAll();
    const onVisibilityChange = () => {
      if (!document.hidden) syncAll();
    };

    const scheduler = createEventRefreshScheduler({
      delayMs: 200,
      isBlocked: () => document.hidden || engineStore.busy,
      onWarning: (message) => engineStore.setNotice(message),
      onFlush: async (plan) => {
        if (plan.status) {
          await engineStore.refresh();
        }
        if (engineStore.state !== "running") return;
        const tasks: Promise<unknown>[] = [];
        if (plan.containers) tasks.push(engineStore.refreshContainers(true));
        if (plan.images) tasks.push(engineStore.refreshImages(true));
        if (plan.volumes) tasks.push(engineStore.refreshVolumes(true));
        if (plan.networks) tasks.push(engineStore.refreshNetworks(true));
        if (tasks.length > 0) {
          await Promise.all(tasks);
        }
      },
    });

    let unlistenDaemon: UnlistenFn | null = null;
    void listen<DaemonEventPayload>("nanodock://daemon-event", (event) => {
      scheduler.handleEvent(event.payload);
    }).then((fn) => {
      unlistenDaemon = fn;
    });

    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      clearInterval(timer);
      scheduler.dispose();
      if (unlistenDaemon) unlistenDaemon();
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  });

  function toggleSidebar() {
    sidebarCollapsed = !sidebarCollapsed;
    localStorage.setItem("nanodock-sidebar-collapsed", String(sidebarCollapsed));
  }

  function navBadge(id: AppTab): string | null {
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

<svelte:window onkeydown={handleGlobalKeydown} />

<div class="flex h-screen bg-slate-50 text-slate-900 transition-colors dark:bg-slate-950 dark:text-slate-100">
  <aside
    class={`flex shrink-0 flex-col justify-between border-r border-slate-200 bg-white transition-all duration-200 dark:border-slate-800 dark:bg-slate-900/60 ${
      sidebarCollapsed ? "w-16 p-2.5" : "w-60 p-4"
    }`}
  >
    <div>
      <div class={`mb-4 flex items-center ${sidebarCollapsed ? "justify-center" : "justify-between px-2"}`}>
        {#if !sidebarCollapsed}
          <span class="text-[11px] font-medium tracking-wide text-slate-400">
            WSL2 Engine{#if appVersion} · v{appVersion}{/if}
          </span>
          <button
            type="button"
            onclick={toggleSidebar}
            title={i18n.t.nav.collapseSidebar}
            aria-label={i18n.t.nav.collapseSidebar}
            class="rounded-md p-1 text-slate-400 transition hover:bg-slate-100 hover:text-slate-600 dark:hover:bg-slate-800 dark:hover:text-slate-200"
          >
            <Icon name="chevron-left" class="h-4 w-4" />
          </button>
        {:else}
          <button
            type="button"
            onclick={toggleSidebar}
            title={i18n.t.nav.expandSidebar}
            aria-label={i18n.t.nav.expandSidebar}
            class="flex w-full items-center justify-center rounded-lg p-2 text-slate-400 transition hover:bg-slate-100 hover:text-slate-600 dark:hover:bg-slate-800 dark:hover:text-slate-200"
          >
            <Icon name="chevron-right" class="h-4 w-4" />
          </button>
        {/if}
      </div>

      <nav class="space-y-1 text-sm" aria-label="Main navigation">
        {#each navItems as item, idx}
          {@const badge = navBadge(item.id)}
          <button
            type="button"
            onclick={() => navigateTo(item.id)}
            title={sidebarCollapsed ? `${item.label} (Ctrl+${idx + 1})` : `Ctrl+${idx + 1}`}
            aria-label={item.label}
            class={`flex w-full items-center rounded-lg py-2 transition ${
              sidebarCollapsed ? "justify-center px-2" : "gap-3 px-3 text-left"
            } ${
              engineStore.activeTab === item.id
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
          onclick={() => (engineStore.state === "stopped" ? engineStore.start() : engineStore.stop())}
          disabled={engineStore.engineBusy}
          title={engineStore.state === "stopped" ? i18n.t.nav.startEngine : i18n.t.nav.hibernateEngine}
          aria-label={engineStore.state === "stopped" ? i18n.t.nav.startEngine : i18n.t.nav.hibernateEngine}
          class="flex w-full items-center justify-center rounded-lg p-2 text-slate-500 transition hover:bg-slate-100 hover:text-sky-600 disabled:opacity-50 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-sky-400"
        >
          {#if engineStore.isPending("engine:start") || engineStore.isPending("engine:stop")}
            <Icon name="spinner" class="h-4 w-4 animate-spin text-sky-500" />
          {:else if engineStore.state === "stopped"}
            <span class="relative flex h-4 w-4 items-center justify-center">
              <Icon name="play" class="h-3.5 w-3.5 text-slate-400" />
            </span>
          {:else}
            <span class="relative flex h-4 w-4 items-center justify-center">
              <span
                class={`h-2.5 w-2.5 rounded-full ${
                  engineStore.state === "running"
                    ? "bg-emerald-500"
                    : engineStore.state === "error"
                      ? "bg-rose-500"
                      : "bg-slate-400"
                }`}
              ></span>
            </span>
          {/if}
        </button>
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
          <div class="flex items-center gap-1.5">
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
            <button
              type="button"
              onclick={() => (engineStore.state === "stopped" ? engineStore.start() : engineStore.stop())}
              disabled={engineStore.engineBusy}
              title={engineStore.state === "stopped" ? i18n.t.nav.startEngine : i18n.t.nav.hibernateEngine}
              aria-label={engineStore.state === "stopped" ? i18n.t.nav.startEngine : i18n.t.nav.hibernateEngine}
              class="rounded-md p-1 text-slate-400 transition hover:bg-slate-100 hover:text-sky-600 disabled:opacity-50 dark:hover:bg-slate-800 dark:hover:text-sky-400"
            >
              {#if engineStore.isPending("engine:start") || engineStore.isPending("engine:stop")}
                <Icon name="spinner" class="h-3.5 w-3.5 animate-spin text-sky-500" />
              {:else}
                <Icon name={engineStore.state === "stopped" ? "play" : "stop"} class="h-3.5 w-3.5" />
              {/if}
            </button>
          </div>
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

  <main
    class={`relative flex-1 overflow-y-auto ${
      engineStore.activeTab === "containers" && engineStore.detailContainerId !== null
        ? "flex flex-col p-3 sm:p-4"
        : "p-7"
    }`}
  >
    {#if engineStore.busy || engineStore.pullingRef || engineStore.loadingResources}
      <div
        aria-hidden="true"
        class="pointer-events-none fixed inset-x-0 top-0 z-50 h-0.5 overflow-hidden bg-sky-500/15"
      >
        <div class="h-full w-1/3 animate-pulse bg-sky-500"></div>
      </div>
    {/if}
    {#if environmentStore.status && !environmentStore.bannerDismissed && (!environmentStore.status.wsl_installed || environmentStore.status.creds_store_warning)}
      <div class="mb-5 flex flex-wrap items-center justify-between gap-3 rounded-xl border border-amber-200 bg-amber-50/80 px-4 py-2.5 text-xs text-amber-900 dark:border-amber-500/30 dark:bg-amber-500/10 dark:text-amber-200">
        <div class="flex items-center gap-2.5">
          <Icon name="warning" class="h-4 w-4 shrink-0 text-amber-600 dark:text-amber-400" />
          <span class="font-medium">
            {!environmentStore.status.wsl_installed
              ? i18n.t.settings.envBannerWslMissing
              : i18n.t.settings.envBannerCredsStore}
          </span>
        </div>
        <div class="flex items-center gap-2">
          {#if !environmentStore.status.wsl_installed}
            <button
              type="button"
              onclick={() => void environmentStore.installWsl2()}
              disabled={environmentStore.busyAction !== null}
              class="inline-flex items-center gap-1.5 rounded-lg bg-amber-600 px-3 py-1.5 font-medium text-white shadow-xs transition hover:bg-amber-500 disabled:opacity-60"
            >
              {environmentStore.busyAction === "wsl2" ? i18n.t.settings.installingWsl2 : i18n.t.settings.installWsl2}
            </button>
          {:else if environmentStore.status.creds_store_warning}
            <button
              type="button"
              onclick={() => void environmentStore.fixCredsStore()}
              disabled={environmentStore.busyAction !== null}
              class="inline-flex items-center gap-1.5 rounded-lg bg-amber-600 px-3 py-1.5 font-medium text-white shadow-xs transition hover:bg-amber-500 disabled:opacity-60"
            >
              {i18n.t.settings.credsWarningFix}
            </button>
          {/if}
          <button
            type="button"
            onclick={() => environmentStore.dismissBanner()}
            class="rounded-lg px-2.5 py-1.5 font-medium text-amber-800 hover:bg-amber-100 dark:text-amber-200 dark:hover:bg-amber-500/20"
          >
            {i18n.t.settings.later}
          </button>
        </div>
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
    {#if engineStore.activeTab === "dashboard"}
      <DashboardView onNavigate={(tab) => navigateTo(tab)} />
    {:else if engineStore.activeTab === "containers"}
      <ContainersView />
    {:else if engineStore.activeTab === "images"}
      <ImagesView />
    {:else if engineStore.activeTab === "volumes"}
      <VolumesView />
    {:else if engineStore.activeTab === "networks"}
      <NetworksView />
    {:else if engineStore.activeTab === "storage"}
      <StorageView />
    {:else}
      <SettingsView />
    {/if}
  </main>

  <!-- Floating Toast Notifications -->
  {#if engineStore.toasts.length > 0}
    <div class="fixed right-5 bottom-5 z-40 flex max-w-md flex-col gap-2">
      {#each engineStore.toasts as toast (toast.id)}
        {#if toast.kind === "error"}
          <div
            role="alert"
            class="animate-toast-in flex items-start justify-between gap-3 rounded-xl border border-rose-200 bg-white/95 px-4 py-3 text-xs text-rose-700 shadow-lg backdrop-blur-xs dark:border-rose-500/30 dark:bg-slate-900/95 dark:text-rose-300"
          >
            <div class="flex items-start gap-2">
              <Icon name="warning" class="mt-0.5 h-3.5 w-3.5 shrink-0 text-rose-500" />
              <span class="leading-relaxed">{toast.message}</span>
            </div>
            <button
              type="button"
              onclick={() => engineStore.dismissToast(toast.id)}
              aria-label="Dismiss error"
              class="shrink-0 rounded p-0.5 text-rose-500 hover:bg-rose-50 dark:hover:bg-rose-500/15"
            >
              <Icon name="close" class="h-3.5 w-3.5" />
            </button>
          </div>
        {:else}
          <div
            role="status"
            class="animate-toast-in flex items-start justify-between gap-3 rounded-xl border border-emerald-200 bg-white/95 px-4 py-3 text-xs text-emerald-700 shadow-lg backdrop-blur-xs dark:border-emerald-500/30 dark:bg-slate-900/95 dark:text-emerald-300"
          >
            <div class="flex items-start gap-2">
              <Icon name="check" class="mt-0.5 h-3.5 w-3.5 shrink-0 text-emerald-500" />
              <span class="leading-relaxed">{toast.message}</span>
            </div>
            <button
              type="button"
              onclick={() => engineStore.dismissToast(toast.id)}
              aria-label="Dismiss notice"
              class="shrink-0 rounded p-0.5 text-emerald-500 hover:bg-emerald-50 dark:hover:bg-emerald-500/15"
            >
              <Icon name="close" class="h-3.5 w-3.5" />
            </button>
          </div>
        {/if}
      {/each}
    </div>
  {/if}
</div>

<QuickGuideModal open={quickGuideOpen} onClose={() => (quickGuideOpen = false)} />
<ConfirmModal
  open={pendingNavTab !== null}
  title={i18n.t.nav.unsavedChangesTitle}
  message={i18n.t.nav.unsavedChangesMsg}
  confirmLabel={i18n.t.nav.discardAndLeave}
  danger={false}
  onConfirm={confirmDiscardSettings}
  onCancel={() => (pendingNavTab = null)}
/>
