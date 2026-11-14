<script lang="ts">
  import { onMount } from "svelte";
  import { showSettings, syncStatus, refreshVaultItems, folderTree } from "../lib/stores";
  import { getAppSettings, setAppSettings, forceResync, clearLocalCache, listFolderTree } from "../lib/tauri";
  import { useEscapeToClose } from "../lib/useEscapeToClose";
  import type { AppSettings } from "../lib/types";

  useEscapeToClose(showSettings);

  let settings = $state<AppSettings>({
    auto_lock_secs: 900,
    clipboard_clear_secs: 30,
  });

  let loading = $state(true);
  let syncing = $state(false);
  let syncMessage = $state("");
  let syncError = $state("");

  const clipboardOptions = [
    { value: 10, label: "10 seconds" },
    { value: 30, label: "30 seconds (Default)" },
    { value: 60, label: "1 minute" },
    { value: 0, label: "Never clear" },
  ];

  onMount(async () => {
    try {
      const s = await getAppSettings();
      settings = s;
    } catch (e) {
      console.warn("Failed to load settings:", e);
    } finally {
      loading = false;
    }
  });

  async function updateClipboard(secs: number) {
    settings.clipboard_clear_secs = secs;
    try {
      await setAppSettings(settings);
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  }

  async function handleForceResync() {
    syncing = true;
    syncMessage = "";
    syncError = "";
    try {
      syncStatus.set("syncing");
      const res = await forceResync();
      await refreshVaultItems();
      folderTree.set(await listFolderTree());
      syncStatus.set("idle");
      syncMessage = `Full re-sync complete: +${res.added} added, ~${res.updated} updated.`;
    } catch (e: unknown) {
      syncStatus.set("idle");
      syncError = e instanceof Error ? e.message : String(e);
    } finally {
      syncing = false;
    }
  }

  async function handleClearCache() {
    syncing = true;
    syncMessage = "";
    syncError = "";
    try {
      syncStatus.set("syncing");
      const res = await clearLocalCache();
      await refreshVaultItems();
      folderTree.set(await listFolderTree());
      syncStatus.set("idle");
      syncMessage = `Cache purged and re-synced: ${res.added} items loaded.`;
    } catch (e: unknown) {
      syncStatus.set("idle");
      syncError = e instanceof Error ? e.message : String(e);
    } finally {
      syncing = false;
    }
  }

  function handleClose() {
    showSettings.set(false);
  }
</script>

{#if $showSettings}
  <div class="fixed inset-x-0 bottom-0 top-9 bg-black/70 flex items-center justify-center z-50">
    <div
      class="bg-gray-900 border border-gray-800 rounded-lg p-6 w-[440px] space-y-5 shadow-2xl text-sm"
      role="dialog"
      aria-modal="true"
    >
      <div class="flex items-center justify-between border-b border-gray-800 pb-3">
        <div class="flex items-center gap-2">
          <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5 text-brand-500" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
          </svg>
          <h2 class="text-base font-semibold text-gray-100">Preferences & Settings</h2>
        </div>
        <button
          onclick={handleClose}
          class="text-gray-400 hover:text-gray-200 transition-colors p-1"
          title="Close"
        >
          <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>

      <!-- Auto-lock policy section -->
      <div class="space-y-1.5">
        <span class="block text-xs uppercase font-medium tracking-wide text-gray-400">
          Vault Lock Policy
        </span>
        <div class="bg-gray-800/60 border border-gray-700/60 rounded p-2.5 space-y-1">
          <div class="flex items-center gap-1.5 text-xs font-medium text-emerald-400">
            <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
              <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
            </svg>
            Auto-lock on reopen & sleep
          </div>
          <p class="text-[11px] text-gray-400">
            The vault automatically locks when the application is reopened or when your computer enters sleep/suspend mode.
          </p>
        </div>
      </div>

      <!-- Clipboard auto-clear section -->
      <div class="space-y-1.5">
        <label for="clipboard-select" class="block text-xs uppercase font-medium tracking-wide text-gray-400">
          Clipboard Auto-Clear
        </label>
        <p class="text-[11px] text-gray-500">Wipe copied passwords from your clipboard after the specified duration.</p>
        <select
          id="clipboard-select"
          value={settings.clipboard_clear_secs}
          onchange={(e) => updateClipboard(parseInt(e.currentTarget.value, 10))}
          class="w-full bg-gray-800 text-gray-100 border border-gray-700 rounded px-3 py-1.5 text-xs outline-none focus:ring-1 focus:ring-brand-500 cursor-pointer"
        >
          {#each clipboardOptions as opt}
            <option value={opt.value} class="bg-gray-900 text-gray-100">{opt.label}</option>
          {/each}
        </select>
      </div>

      <!-- Cache & Sync section -->
      <div class="space-y-2 pt-2 border-t border-gray-800">
        <p class="text-xs uppercase font-medium tracking-wide text-gray-400">
          Sync & Local Storage
        </p>
        <div class="flex items-center gap-2">
          <button
            type="button"
            onclick={handleForceResync}
            disabled={syncing}
            class="flex-1 px-3 py-1.5 bg-gray-800 hover:bg-gray-700 disabled:opacity-40 text-gray-200 border border-gray-700 rounded text-xs font-medium transition-colors"
          >
            {syncing ? "Syncing..." : "Force Re-sync"}
          </button>
          <button
            type="button"
            onclick={handleClearCache}
            disabled={syncing}
            class="flex-1 px-3 py-1.5 bg-gray-800 hover:bg-gray-700 disabled:opacity-40 text-brand-400 hover:text-brand-300 border border-gray-700 rounded text-xs font-medium transition-colors"
          >
            {syncing ? "Purging..." : "Clear Cache & Re-sync"}
          </button>
        </div>
        {#if syncMessage}
          <p class="text-xs text-green-400 font-medium">{syncMessage}</p>
        {/if}
        {#if syncError}
          <p class="text-xs text-brand-400">{syncError}</p>
        {/if}
      </div>

      <div class="flex justify-end pt-2 border-t border-gray-800">
        <button
          type="button"
          onclick={handleClose}
          class="px-4 py-1.5 bg-brand-600 hover:bg-brand-500 text-white rounded text-xs font-medium transition-colors"
        >
          Done
        </button>
      </div>
    </div>
  </div>
{/if}
