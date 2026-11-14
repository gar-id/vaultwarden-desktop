<script lang="ts">
  import { get } from "svelte/store";
  import {
    profiles,
    activeProfileId,
    vaultLocked,
    showAddProfile,
    showSettings,
    resources,
    folderTree,
    selectedFolderId,
    selectedResource,
    searchQuery,
    refreshVaultItems,
    theme,
    toggleTheme,
  } from "../lib/stores";
  import {
    switchProfile,
    lockVault,
    searchResources,
    listFolderTree,
    syncVault,
  } from "../lib/tauri";
  import ProfileContextMenu from "./ProfileContextMenu.svelte";

  async function handleSwitch(id: string) {
    // Clicking the already-active profile is a no-op.
    if (get(activeProfileId) === id) return;

    const status = await switchProfile(id);
    activeProfileId.set(id);

    if (status === "unlocked") {
      vaultLocked.set(false);
      selectedFolderId.set(null);
      selectedResource.set(null);
      searchQuery.set("");
      resources.set(await searchResources("", null));
      folderTree.set(await listFolderTree());

      // Background delta sync for this profile without blocking UI
      syncVault()
        .then(async () => {
          await refreshVaultItems();
          folderTree.set(await listFolderTree());
        })
        .catch(() => {});
    } else {
      vaultLocked.set(true);
      selectedFolderId.set(null);
      selectedResource.set(null);
      searchQuery.set("");
      resources.set([]);
      folderTree.set([]);
    }
  }

  async function handleLock() {
    await lockVault();
    vaultLocked.set(true);
    selectedResource.set(null);
  }

  let contextMenu = $state<{ profileId: string; profileName: string; x: number; y: number } | null>(null);

  function handleContextMenu(e: MouseEvent, profileId: string, profileName: string) {
    e.preventDefault();
    contextMenu = { profileId, profileName, x: e.clientX, y: e.clientY };
  }
</script>

<aside class="flex flex-col items-center w-14 bg-gray-900 border-r border-gray-800 py-3 gap-2 shrink-0">
  {#each $profiles as profile}
    <button
      onclick={() => handleSwitch(profile.id)}
      oncontextmenu={(e) => handleContextMenu(e, profile.id, profile.name)}
      title="{profile.name} (right-click to remove)"
      class="w-9 h-9 rounded-full bg-brand-600 hover:bg-brand-500 flex items-center justify-center font-bold text-xs uppercase transition-colors
             {$activeProfileId === profile.id ? 'ring-2 ring-brand-400' : ''}"
    >
      {profile.name[0]}
    </button>
  {/each}

  {#if contextMenu}
    <ProfileContextMenu
      profileId={contextMenu.profileId}
      profileName={contextMenu.profileName}
      x={contextMenu.x}
      y={contextMenu.y}
      onClose={() => (contextMenu = null)}
    />
  {/if}

  <button
    onclick={() => showAddProfile.set(true)}
    title="Add server"
    class="w-9 h-9 rounded-full bg-gray-700 hover:bg-gray-600 flex items-center justify-center text-gray-300 text-lg transition-colors mt-auto"
  >+</button>

  <button
    onclick={toggleTheme}
    title={$theme === "dark" ? "Switch to light mode" : "Switch to dark mode"}
    class="w-9 h-9 rounded-full bg-gray-800 hover:bg-gray-700 flex items-center justify-center text-gray-400 hover:text-gray-200 transition-colors"
  >
    {#if $theme === "dark"}
      <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="5" />
        <line x1="12" y1="1" x2="12" y2="3" />
        <line x1="12" y1="21" x2="12" y2="23" />
        <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
        <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
        <line x1="1" y1="12" x2="3" y2="12" />
        <line x1="21" y1="12" x2="23" y2="12" />
        <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
        <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
      </svg>
    {:else}
      <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
      </svg>
    {/if}
  </button>

  <button
    onclick={() => showSettings.set(true)}
    title="Preferences & Settings"
    class="w-9 h-9 rounded-full bg-gray-800 hover:bg-gray-700 flex items-center justify-center text-gray-400 hover:text-gray-200 transition-colors"
  >
    <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  </button>

  <button
    onclick={handleLock}
    title="Lock vault (Ctrl+L)"
    class="w-9 h-9 rounded-full bg-gray-800 hover:bg-red-900 flex items-center justify-center text-gray-400 hover:text-red-300 transition-colors"
  >
    <!-- lock icon -->
    <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <rect x="3" y="11" width="18" height="11" rx="2" ry="2"/>
      <path d="M7 11V7a5 5 0 0 1 10 0v4"/>
    </svg>
  </button>
</aside>
