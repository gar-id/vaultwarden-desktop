<script lang="ts">
  import { showAddProfile, profiles } from "../lib/stores";
  import { addProfile, listProfiles } from "../lib/tauri";
  import { useEscapeToClose } from "../lib/useEscapeToClose";

  useEscapeToClose(showAddProfile);

  let serverType = $state<"us" | "eu" | "custom">("us");
  let showPassword = $state(false);
  let form = $state({
    name: "",
    server_url: "https://vault.bitwarden.com",
    user_email: "",
    master_password: "",
  });
  let error = $state("");
  let loading = $state(false);

  function handleServerTypeChange(type: "us" | "eu" | "custom") {
    serverType = type;
    if (type === "us") {
      form.server_url = "https://vault.bitwarden.com";
      if (!form.name || form.name === "Bitwarden EU" || form.name === "Self-Hosted") form.name = "Bitwarden US";
    } else if (type === "eu") {
      form.server_url = "https://vault.bitwarden.eu";
      if (!form.name || form.name === "Bitwarden US" || form.name === "Self-Hosted") form.name = "Bitwarden EU";
    } else {
      form.server_url = "";
      if (!form.name || form.name.startsWith("Bitwarden")) form.name = "Self-Hosted Vault";
    }
  }

  async function handleSubmit() {
    if (!form.user_email || !form.master_password || !form.server_url) {
      error = "All fields are required";
      return;
    }
    loading = true;
    error = "";
    try {
      await addProfile({
        name: form.name.trim() || "My Vault",
        server_url: form.server_url.trim(),
        user_email: form.user_email.trim(),
        master_password: form.master_password,
        armored_user_private_key: form.master_password, // fallback
      });
      profiles.set(await listProfiles());
      showAddProfile.set(false);
      form = { name: "", server_url: "https://vault.bitwarden.com", user_email: "", master_password: "" };
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }
</script>

{#if $showAddProfile}
  <div class="fixed inset-x-0 bottom-0 top-9 bg-black/70 flex items-center justify-center z-50">
    <form onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}
          class="bg-gray-900 rounded-lg p-6 w-[480px] space-y-4 shadow-2xl border border-gray-800">
      <h2 class="text-lg font-semibold text-gray-100">Add Account</h2>

      <div>
        <span class="text-xs text-gray-400 uppercase tracking-wide block mb-1.5">Server</span>
        <div class="grid grid-cols-3 gap-2">
          <button type="button"
            onclick={() => handleServerTypeChange("us")}
            class="py-1.5 px-3 rounded text-xs font-medium border transition-colors {serverType === 'us' ? 'bg-brand-600/20 border-brand-500 text-brand-300' : 'bg-gray-800 border-gray-700 text-gray-400 hover:text-gray-200'}">
            Bitwarden (US)
          </button>
          <button type="button"
            onclick={() => handleServerTypeChange("eu")}
            class="py-1.5 px-3 rounded text-xs font-medium border transition-colors {serverType === 'eu' ? 'bg-brand-600/20 border-brand-500 text-brand-300' : 'bg-gray-800 border-gray-700 text-gray-400 hover:text-gray-200'}">
            Bitwarden (EU)
          </button>
          <button type="button"
            onclick={() => handleServerTypeChange("custom")}
            class="py-1.5 px-3 rounded text-xs font-medium border transition-colors {serverType === 'custom' ? 'bg-brand-600/20 border-brand-500 text-brand-300' : 'bg-gray-800 border-gray-700 text-gray-400 hover:text-gray-200'}">
            Self-Hosted
          </button>
        </div>
      </div>

      {#if serverType === "custom"}
        <label class="block">
          <span class="text-xs text-gray-400 uppercase tracking-wide">Server URL</span>
          <input type="url" placeholder="https://vaultwarden.example.com" bind:value={form.server_url}
            class="mt-1 w-full bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500" />
        </label>
      {/if}

      <label class="block">
        <span class="text-xs text-gray-400 uppercase tracking-wide">Account Name</span>
        <input type="text" placeholder="Personal Vault" bind:value={form.name}
          class="mt-1 w-full bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500" />
      </label>

      <label class="block">
        <span class="text-xs text-gray-400 uppercase tracking-wide">Email Address</span>
        <input type="email" placeholder="you@example.com" bind:value={form.user_email}
          class="mt-1 w-full bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500" />
      </label>

      <label class="block">
        <span class="text-xs text-gray-400 uppercase tracking-wide">Master Password</span>
        <div class="relative mt-1 flex items-center">
          <input
            type={showPassword ? "text" : "password"}
            placeholder="••••••••••••"
            bind:value={form.master_password}
            class="w-full bg-gray-800 text-gray-100 placeholder-gray-600 rounded pl-3 pr-10 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500 font-mono"
          />
          <button
            type="button"
            onclick={() => (showPassword = !showPassword)}
            title={showPassword ? "Hide password" : "Show password"}
            class="absolute right-2 text-gray-400 hover:text-gray-200 transition-colors p-1"
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
      </label>

      {#if error}
        <p class="text-red-400 text-xs">{error}</p>
      {/if}

      <div class="flex justify-end gap-2 pt-2">
        <button type="button" onclick={() => showAddProfile.set(false)}
          class="px-4 py-1.5 text-sm text-gray-400 hover:text-gray-200 transition-colors">Cancel</button>
        <button type="submit" disabled={loading}
          class="px-4 py-1.5 text-sm bg-brand-600 hover:bg-brand-500 disabled:opacity-50 rounded font-medium transition-colors text-white">
          {loading ? "Connecting..." : "Add Account"}
        </button>
      </div>
    </form>
  </div>
{/if}
