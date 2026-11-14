<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { error as logError } from "@tauri-apps/plugin-log";
  import { listProfiles, switchProfile, lockVault } from "./lib/tauri";
  import { profiles, vaultLocked, activeProfileId, resourceFormMode, selectedResource } from "./lib/stores";
  import TitleBar from "./components/TitleBar.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import FolderTree from "./components/FolderTree.svelte";
  import VaultList from "./components/VaultList.svelte";
  import DetailPane from "./components/DetailPane.svelte";
  import LockScreen from "./components/LockScreen.svelte";
  import AddProfile from "./components/AddProfile.svelte";
  import PasswordGen from "./components/PasswordGen.svelte";
  import ResourceForm from "./components/ResourceForm.svelte";
  import ShareModal from "./components/ShareModal.svelte";
  import SettingsModal from "./components/SettingsModal.svelte";

  function handleGlobalKeydown(e: KeyboardEvent) {
    const isMac = typeof navigator !== "undefined" && navigator.platform.toUpperCase().indexOf("MAC") >= 0;
    const modifier = isMac ? e.metaKey : e.ctrlKey;

    if (modifier && e.key.toLowerCase() === "f") {
      e.preventDefault();
      const searchInput = document.querySelector<HTMLInputElement>("input[placeholder*='Search vault']");
      if (searchInput) {
        searchInput.focus();
        searchInput.select();
      }
    } else if (modifier && e.key.toLowerCase() === "n") {
      e.preventDefault();
      let locked = true;
      const unsub = vaultLocked.subscribe((v) => (locked = v));
      unsub();
      if (!locked) {
        resourceFormMode.set("create");
      }
    } else if (modifier && e.key.toLowerCase() === "l") {
      e.preventDefault();
      lockVault()
        .then(() => {
          vaultLocked.set(true);
          selectedResource.set(null);
        })
        .catch(() => {});
    }
  }

  let unlistenAutoLock: (() => void) | undefined;

  onDestroy(() => {
    if (unlistenAutoLock) unlistenAutoLock();
  });

  onMount(async () => {
    try {
      unlistenAutoLock = await listen("vault_auto_locked", () => {
        vaultLocked.set(true);
        selectedResource.set(null);
      });

      const list = await listProfiles();
      profiles.set(list);
      if (list.length > 0) {
        // Must call the real switch_profile IPC command, not just set the
        // local store - activeProfileId here only controls which avatar
        // shows the selection ring. Without this call, the backend's
        // AppState.active_profile_id stays None (it resets on every
        // restart, it's not persisted), so unlock_vault fails with "No
        // active profile" even though the sidebar already shows one
        // selected. It also made the profile icon look unclickable: the
        // sidebar's "already active, no-op" guard matched immediately
        // since the frontend store disagreed with the real backend state.
        const status = await switchProfile(list[0].id);
        activeProfileId.set(list[0].id);
        vaultLocked.set(status === "locked");
      }
    } catch (e) {
      // logError writes to the same Rust-side log file/stdout as the
      // backend, not just this window's DevTools console - so this failure
      // leaves a trail on disk even if nobody had DevTools open.
      void logError(`Failed to load profiles on mount: ${e}`);
    }
  });
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<div class="flex flex-col h-screen w-screen overflow-hidden bg-gray-950 text-gray-100 text-sm">
  <TitleBar />

  <div class="flex flex-1 min-h-0">
    <Sidebar />

    {#if $vaultLocked}
      <LockScreen />
    {:else}
      <!-- Three columns: folder tree | secret list | details. min-h-0 here
           is required for the scroll fixes inside each column to actually
           work - a flex row's children default to a min-height equal to
           their content, which silently overrides any overflow-y-auto
           further down the tree and was the real cause of "tree can't
           scroll". -->
      <div class="flex flex-1 min-h-0">
        <FolderTree />
        <VaultList />
        <DetailPane />
      </div>
    {/if}
  </div>

  <AddProfile />
  <PasswordGen />
  <ResourceForm />
  <ShareModal />
  <SettingsModal />
</div>
