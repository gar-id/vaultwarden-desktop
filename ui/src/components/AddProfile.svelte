<script lang="ts">
  import { showAddProfile, profiles } from "../lib/stores";
  import { addProfile, listProfiles } from "../lib/tauri";
  import { useEscapeToClose } from "../lib/useEscapeToClose";

  useEscapeToClose(showAddProfile);

  let step = $state<"credentials" | "two_factor">("credentials");
  let serverType = $state<"us" | "eu" | "custom">("us");
  let showPassword = $state(false);

  let form = $state({
    name: "",
    server_url: "https://vault.bitwarden.com",
    user_email: "",
    master_password: "",
  });

  let twoFactorCode = $state("");
  let savedDeviceId = $state("");
  let twoFactorProvider = $state(0); // 0 = Authenticator, 1 = Email
  let error = $state("");
  let loading = $state(false);

  function handleServerTypeChange(type: "us" | "eu" | "custom") {
    serverType = type;
    if (type === "us") {
      form.server_url = "https://vault.bitwarden.com";
      if (!form.name || form.name === "Bitwarden EU" || form.name === "Self-Hosted Vault") form.name = "Bitwarden US";
    } else if (type === "eu") {
      form.server_url = "https://vault.bitwarden.eu";
      if (!form.name || form.name === "Bitwarden US" || form.name === "Self-Hosted Vault") form.name = "Bitwarden EU";
    } else {
      form.server_url = "";
      if (!form.name || form.name.startsWith("Bitwarden")) form.name = "Self-Hosted Vault";
    }
  }

  function resetForm() {
    step = "credentials";
    error = "";
    twoFactorCode = "";
    savedDeviceId = "";
    twoFactorProvider = 0;
    form = { name: "", server_url: "https://vault.bitwarden.com", user_email: "", master_password: "" };
  }

  async function handleSubmit() {
    if (!form.user_email || !form.master_password || !form.server_url) {
      error = "All fields are required";
      return;
    }

    loading = true;
    error = "";

    console.log("[AddProfile] Submitting login request:", {
      serverType,
      url: form.server_url,
      email: form.user_email.trim(),
      pwdLen: form.master_password.length,
      step,
      twoFactorTokenLen: twoFactorCode.trim().length,
    });

    try {
      await addProfile({
        name: form.name.trim() || "My Vault",
        server_url: form.server_url.trim(),
        user_email: form.user_email.trim(),
        master_password: form.master_password,
        armored_user_private_key: form.master_password, // fallback
        two_factor_provider: step === "two_factor" ? twoFactorProvider : undefined,
        two_factor_token: step === "two_factor" && twoFactorCode.trim() ? twoFactorCode.trim() : undefined,
        two_factor_remember: true,
        device_id: savedDeviceId || undefined,
      });

      console.log("[AddProfile] Login successful!");
      profiles.set(await listProfiles());
      showAddProfile.set(false);
      resetForm();
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error("[AddProfile] Login error received:", msg);

      if (msg.includes("2FA_REQUIRED")) {
        // Backend format: "2FA_REQUIRED:<device_id>:<providers_comma_separated>"
        const parts = msg.split(":");
        savedDeviceId = parts[1] || "";
        const provs = (parts[2] || "0").split(",").map((p) => parseInt(p, 10)).filter((p) => !isNaN(p));
        // Default to Authenticator (0) if supported, else Email (1)
        twoFactorProvider = provs.includes(0) ? 0 : provs.length > 0 ? provs[0] : 0;
        step = "two_factor";
        twoFactorCode = "";
        error = "";
      } else {
        error = msg.replace(/^Login failed:\s*/i, "");
      }
    } finally {
      loading = false;
    }
  }
</script>

{#if $showAddProfile}
  <div class="fixed inset-x-0 bottom-0 top-9 bg-black/70 flex items-center justify-center z-50">
    <div class="bg-gray-900 rounded-lg p-6 w-[480px] shadow-2xl border border-gray-800">
      {#if step === "credentials"}
        <form onsubmit={(e) => { e.preventDefault(); handleSubmit(); }} class="space-y-4">
          <div class="flex items-center justify-between">
            <h2 class="text-lg font-semibold text-gray-100">Add Account</h2>
            <img src="/vaultwarden-light.svg" alt="" class="w-5 h-5 opacity-70" />
          </div>

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
            <button type="button" onclick={() => { showAddProfile.set(false); resetForm(); }}
              class="px-4 py-1.5 text-sm text-gray-400 hover:text-gray-200 transition-colors">Cancel</button>
            <button type="submit" disabled={loading}
              class="px-4 py-1.5 text-sm bg-brand-600 hover:bg-brand-500 disabled:opacity-50 rounded font-medium transition-colors text-white">
              {loading ? "Connecting..." : "Add Account"}
            </button>
          </div>
        </form>
      {:else if step === "two_factor"}
        <form onsubmit={(e) => { e.preventDefault(); handleSubmit(); }} class="space-y-4">
          <div class="flex items-center gap-2">
            <div class="w-8 h-8 rounded-full bg-brand-600/20 text-brand-400 flex items-center justify-center font-bold text-sm">
              2FA
            </div>
            <div>
              <h2 class="text-base font-semibold text-gray-100">Two-Step Verification</h2>
              <p class="text-xs text-gray-400">{form.user_email}</p>
            </div>
          </div>

          <p class="text-xs text-gray-300 leading-relaxed">
            {#if twoFactorProvider === 1}
              A verification code was sent to your email address. Enter the 6-digit code below to finish connecting your account:
            {:else}
              Enter the 6-digit verification code generated by your Authenticator app (e.g. Google Authenticator, Authy, Aegis):
            {/if}
          </p>

          <label class="block">
            <span class="text-xs text-gray-400 uppercase tracking-wide block mb-1">Verification Code</span>
            <input
              type="text"
              inputmode="numeric"
              autocomplete="one-time-code"
              maxlength="10"
              placeholder="123456"
              bind:value={twoFactorCode}
              class="w-full bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-3 py-2 text-center text-xl tracking-widest font-mono outline-none focus:ring-1 focus:ring-brand-500"
            />
          </label>

          {#if error}
            <p class="text-red-400 text-xs">{error}</p>
          {/if}

          <div class="flex justify-between items-center pt-2">
            <button
              type="button"
              onclick={() => { step = "credentials"; error = ""; }}
              class="text-xs text-gray-400 hover:text-gray-200 transition-colors"
            >
              ← Back to password
            </button>
            <div class="flex gap-2">
              <button
                type="button"
                onclick={() => { showAddProfile.set(false); resetForm(); }}
                class="px-3 py-1.5 text-sm text-gray-400 hover:text-gray-200 transition-colors"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={loading || !twoFactorCode.trim()}
                class="px-4 py-1.5 text-sm bg-brand-600 hover:bg-brand-500 disabled:opacity-50 rounded font-medium transition-colors text-white"
              >
                {loading ? "Verifying..." : "Verify & Log In"}
              </button>
            </div>
          </div>
        </form>
      {/if}
    </div>
  </div>
{/if}
