<script lang="ts">
  import { unlockVault, syncVault } from "../lib/tauri";
  import { error as logError } from "@tauri-apps/plugin-log";
  import { vaultLocked, syncStatus, refreshVaultItems, theme } from "../lib/stores";

  let passphrase = $state("");
  let showPassword = $state(false);
  let error = $state("");
  let loading = $state(false);

  async function handleUnlock() {
    if (!passphrase) return;
    loading = true;
    error = "";
    try {
      const result = await unlockVault(passphrase);
      if (result.success) {
        vaultLocked.set(false);

        if (result.online) {
          syncStatus.set("syncing");
          try {
            await syncVault();
          } catch (e: unknown) {
            void logError(`sync_vault after unlock failed: ${e}`);
          }
        } else {
          syncStatus.set("offline");
        }

        await refreshVaultItems();
        syncStatus.set("idle");
      } else {
        error = result.message;
      }
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }
</script>

<div class="flex-1 flex items-center justify-center bg-gray-950">
  <form onsubmit={(e) => { e.preventDefault(); handleUnlock(); }}
        class="w-80 space-y-4">
    <div class="flex justify-center mb-2">
      <img src={$theme === "light" ? "/vaultwarden.svg" : "/vaultwarden-light.svg"} alt="Vaultwarden" class="w-16 h-16" onerror={(e) => { (e.currentTarget as HTMLElement).style.display = 'none'; }} />
    </div>
    <h1 class="text-2xl font-semibold text-gray-100 text-center">Unlock Vault</h1>
    <p class="text-sm text-gray-500 text-center">Enter your master password</p>

    <div class="relative flex items-center">
      <input
        type={showPassword ? "text" : "password"}
        bind:value={passphrase}
        placeholder="Master Password"
        class="w-full bg-gray-800 text-gray-100 placeholder-gray-500 rounded pl-3 pr-10 py-2 outline-none focus:ring-1 focus:ring-brand-500 font-mono"
      />
      <button
        type="button"
        onclick={() => (showPassword = !showPassword)}
        title={showPassword ? "Hide password" : "Show password"}
        class="absolute right-2.5 text-gray-400 hover:text-gray-200 transition-colors p-1"
      >
        {#if showPassword}
          <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
            <line x1="1" y1="1" x2="23" y2="23" />
          </svg>
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
            <circle cx="12" cy="12" r="3" />
          </svg>
        {/if}
      </button>
    </div>

    {#if error}
      <p class="text-red-400 text-xs text-center">{error}</p>
    {/if}

    <button type="submit" disabled={loading}
      class="w-full py-2 bg-brand-600 hover:bg-brand-500 disabled:opacity-50 rounded font-medium transition-colors text-white">
      {loading ? "Unlocking..." : "Unlock"}
    </button>
  </form>
</div>
