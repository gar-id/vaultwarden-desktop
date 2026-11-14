<script lang="ts">
  import { removeProfile, listProfiles, switchProfile } from "../lib/tauri";
  import { profiles, activeProfileId, vaultLocked } from "../lib/stores";
  import { error as logError } from "@tauri-apps/plugin-log";

  interface Props {
    profileId: string;
    profileName: string;
    x: number;
    y: number;
    onClose: () => void;
  }

  let { profileId, profileName, x, y, onClose }: Props = $props();

  let confirming = $state(false);
  let deleting = $state(false);
  let error = $state("");

  async function handleConfirmDelete() {
    deleting = true;
    error = "";
    try {
      await removeProfile(profileId);

      // remove_profile clears the backend's active_profile_id/db/session
      // too if this was the active profile - mirror that here so the UI
      // doesn't keep showing a vault view for a profile that's gone.
      const wasActive = profileId === profileIdSnapshot();
      const list = await listProfiles();
      profiles.set(list);

      if (wasActive) {
        if (list.length > 0) {
          const status = await switchProfile(list[0].id);
          activeProfileId.set(list[0].id);
          vaultLocked.set(status === "locked");
        } else {
          activeProfileId.set(null);
          vaultLocked.set(true);
        }
      }

      onClose();
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
      deleting = false;
    }
  }

  function profileIdSnapshot(): string | null {
    let current: string | null = null;
    const unsubscribe = activeProfileId.subscribe((v) => (current = v));
    unsubscribe();
    return current;
  }
</script>

<!-- Invisible full-screen backdrop to catch outside clicks and close the menu -->
<div
  class="fixed inset-0 z-40"
  onclick={onClose}
  oncontextmenu={(e) => { e.preventDefault(); onClose(); }}
  role="presentation"
></div>

<div
  class="fixed z-50 bg-gray-800 border border-gray-700 rounded shadow-xl py-1 w-56 text-sm"
  style="left: {x}px; top: {y}px;"
>
  {#if !confirming}
    <button
      onclick={() => (confirming = true)}
      class="w-full text-left px-3 py-2 text-red-400 hover:bg-gray-700 transition-colors"
    >
      Delete "{profileName}"
    </button>
  {:else}
    <div class="px-3 py-2 space-y-2">
      <p class="text-gray-300 text-xs">
        Remove "{profileName}" from this device? Your Vaultwarden/Bitwarden account and data on the server are not affected — you can re-add it anytime.
      </p>
      {#if error}
        <p class="text-red-400 text-xs">{error}</p>
      {/if}
      <div class="flex justify-end gap-2 pt-1">
        <button
          onclick={onClose}
          disabled={deleting}
          class="px-2 py-1 text-xs text-gray-400 hover:text-gray-200 transition-colors"
        >Cancel</button>
        <button
          onclick={handleConfirmDelete}
          disabled={deleting}
          class="px-2 py-1 text-xs bg-red-600 hover:bg-red-500 disabled:opacity-50 rounded font-medium transition-colors"
        >{deleting ? "Removing..." : "Remove"}</button>
      </div>
    </div>
  {/if}
</div>
