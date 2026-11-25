<script lang="ts">
  import { onMount } from "svelte";
  import { daemonApi } from "../../lib/api/daemon";
  import { engineStore } from "../../lib/stores/engine.svelte";

  onMount(() => {
    engineStore.loadConfig();
  });

  async function save() {
    if (!engineStore.config) return;
    const resp = await daemonApi.updateConfig(engineStore.config);
    if (resp.status === "ack") {
      engineStore.notice = "Settings saved.";
    } else if (resp.status === "error") {
      engineStore.error = resp.data.message;
    }
  }
</script>

<section class="space-y-6">
  <h2 class="text-lg font-semibold text-slate-900 dark:text-white">Engine & WSL2 Settings</h2>

  {#if engineStore.config}
    <div class="space-y-5 rounded-2xl border border-slate-200 bg-white p-6 shadow-xs dark:border-slate-800 dark:bg-slate-900/60">
      <div>
        <label for="ram-range" class="flex justify-between text-sm text-slate-700 dark:text-slate-300">
          <span>WSL2 Memory Limit</span>
          <span class="font-mono font-medium text-sky-700 dark:text-sky-400">
            {engineStore.config.wsl.memory_mb} MB
          </span>
        </label>
        <input
          id="ram-range"
          type="range"
          min="2048"
          max="16384"
          step="512"
          bind:value={engineStore.config.wsl.memory_mb}
          class="mt-2 w-full accent-sky-600"
        />
      </div>

      <div>
        <label for="swap-range" class="flex justify-between text-sm text-slate-700 dark:text-slate-300">
          <span>Swap (Anti-OOM)</span>
          <span class="font-mono font-medium text-sky-700 dark:text-sky-400">
            {engineStore.config.wsl.swap_mb} MB
          </span>
        </label>
        <input
          id="swap-range"
          type="range"
          min="1024"
          max="8192"
          step="512"
          bind:value={engineStore.config.wsl.swap_mb}
          class="mt-2 w-full accent-sky-600"
        />
      </div>

      <label class="flex items-center gap-3 text-sm text-slate-700 dark:text-slate-300">
        <input type="checkbox" bind:checked={engineStore.config.auto_trim_on_events} class="accent-sky-600" />
        Auto-run fstrim on container/image delete
      </label>

      <button
        onclick={save}
        class="rounded-lg bg-sky-600 px-4 py-2 text-sm font-medium text-white shadow-xs transition hover:bg-sky-500"
      >
        Save Settings
      </button>
    </div>
  {:else}
    <p class="text-sm text-slate-500 dark:text-slate-400">Connect core-daemon to load configuration.</p>
  {/if}
</section>
