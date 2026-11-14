<script lang="ts">
  import { unlockVault, syncVault } from "../lib/tauri";
  import { error as logError } from "@tauri-apps/plugin-log";
  import { vaultLocked, syncStatus, refreshVaultItems } from "../lib/stores";

  let passphrase = $state("");
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
      <img src="/vaultwarden.svg" alt="Vaultwarden" class="w-16 h-16" onerror={(e) => { (e.currentTarget as HTMLElement).style.display = 'none'; }} />
    </div>
    <h1 class="text-2xl font-semibold text-gray-100 text-center">Unlock Vault</h1>
    <p class="text-sm text-gray-500 text-center">Enter your master password</p>

    <input
      type="password"
      bind:value={passphrase}
      placeholder="Master Password"
      class="w-full bg-gray-800 text-gray-100 placeholder-gray-500 rounded px-3 py-2 outline-none focus:ring-1 focus:ring-brand-500"
    />

    {#if error}
      <p class="text-red-400 text-xs text-center">{error}</p>
    {/if}

    <button type="submit" disabled={loading}
      class="w-full py-2 bg-brand-600 hover:bg-brand-500 disabled:opacity-50 rounded font-medium transition-colors text-white">
      {loading ? "Unlocking..." : "Unlock"}
    </button>
  </form>
</div>
