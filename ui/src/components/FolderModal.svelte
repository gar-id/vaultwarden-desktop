<script lang="ts">
  import { createFolder, renameFolder, deleteFolder, listFolderTree, searchResources } from "../lib/tauri";
  import { folderTree, selectedFolderId, resources } from "../lib/stores";

  export type FolderModalState =
    | { action: "create"; parentId?: string | null; parentName?: string }
    | { action: "rename"; folderId: string; currentName: string }
    | { action: "delete"; folderId: string; folderName: string }
    | null;

  interface Props {
    modal: FolderModalState;
    onClose: () => void;
  }

  let { modal, onClose }: Props = $props();

  let name = $state("");
  let loading = $state(false);
  let error = $state("");

  $effect(() => {
    error = "";
    loading = false;
    if (modal?.action === "rename") {
      name = modal.currentName;
    } else {
      name = "";
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && !loading) {
      onClose();
    }
  }

  async function handleSubmit() {
    if (!modal) return;
    error = "";

    if (modal.action === "create") {
      if (!name.trim()) {
        error = "Folder name cannot be empty";
        return;
      }
      loading = true;
      try {
        await createFolder(name.trim(), modal.parentId ?? null);
        folderTree.set(await listFolderTree());
        onClose();
      } catch (e: unknown) {
        error = e instanceof Error ? e.message : String(e);
      } finally {
        loading = false;
      }
    } else if (modal.action === "rename") {
      if (!name.trim()) {
        error = "Folder name cannot be empty";
        return;
      }
      loading = true;
      try {
        await renameFolder(modal.folderId, name.trim());
        folderTree.set(await listFolderTree());
        onClose();
      } catch (e: unknown) {
        error = e instanceof Error ? e.message : String(e);
      } finally {
        loading = false;
      }
    } else if (modal.action === "delete") {
      loading = true;
      try {
        await deleteFolder(modal.folderId);
        folderTree.set(await listFolderTree());

        // If the currently selected folder was deleted, reset to My Workspace
        let currentFolder: string | null = null;
        const unsub = selectedFolderId.subscribe((v) => (currentFolder = v));
        unsub();
        if (currentFolder === modal.folderId) {
          selectedFolderId.set(null);
          resources.set(await searchResources("", null));
        }

        onClose();
      } catch (e: unknown) {
        error = e instanceof Error ? e.message : String(e);
      } finally {
        loading = false;
      }
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if modal}
  <div class="fixed inset-x-0 bottom-0 top-9 bg-black/70 flex items-center justify-center z-50">
    <div
      class="bg-gray-900 border border-gray-800 rounded-lg p-5 w-80 space-y-4 shadow-2xl text-sm"
      role="dialog"
      aria-modal="true"
    >
      {#if modal.action === "create"}
        <h2 class="text-base font-semibold text-gray-100">
          {modal.parentName ? `New Subfolder in "${modal.parentName}"` : "New Folder"}
        </h2>
        <form onsubmit={(e) => { e.preventDefault(); handleSubmit(); }} class="space-y-3">
          <label class="block">
            <span class="text-xs text-gray-400 uppercase tracking-wide">Name</span>
            <!-- svelte-ignore a11y_autofocus -->
            <input
              type="text"
              bind:value={name}
              placeholder="e.g. Infrastructure"
              autofocus
              class="mt-1 w-full bg-gray-800 text-gray-100 placeholder-gray-500 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500"
            />
          </label>
          {#if error}
            <p class="text-brand-400 text-xs">{error}</p>
          {/if}
          <div class="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onclick={onClose}
              disabled={loading}
              class="px-3 py-1.5 text-xs text-gray-400 hover:text-gray-200 transition-colors"
            >Cancel</button>
            <button
              type="submit"
              disabled={loading}
              class="px-3 py-1.5 text-xs bg-brand-600 hover:bg-brand-500 disabled:opacity-50 rounded font-medium transition-colors"
            >{loading ? "Creating..." : "Create"}</button>
          </div>
        </form>
      {:else if modal.action === "rename"}
        <h2 class="text-base font-semibold text-gray-100">Rename Folder</h2>
        <form onsubmit={(e) => { e.preventDefault(); handleSubmit(); }} class="space-y-3">
          <label class="block">
            <span class="text-xs text-gray-400 uppercase tracking-wide">Name</span>
            <!-- svelte-ignore a11y_autofocus -->
            <input
              type="text"
              bind:value={name}
              autofocus
              class="mt-1 w-full bg-gray-800 text-gray-100 placeholder-gray-500 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500"
            />
          </label>
          {#if error}
            <p class="text-brand-400 text-xs">{error}</p>
          {/if}
          <div class="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onclick={onClose}
              disabled={loading}
              class="px-3 py-1.5 text-xs text-gray-400 hover:text-gray-200 transition-colors"
            >Cancel</button>
            <button
              type="submit"
              disabled={loading}
              class="px-3 py-1.5 text-xs bg-brand-600 hover:bg-brand-500 disabled:opacity-50 rounded font-medium transition-colors"
            >{loading ? "Saving..." : "Save"}</button>
          </div>
        </form>
      {:else if modal.action === "delete"}
        <h2 class="text-base font-semibold text-gray-100">Delete Folder</h2>
        <div class="space-y-3">
          <p class="text-gray-300 text-xs leading-relaxed">
            Delete <span class="font-semibold text-gray-100">"{modal.folderName}"</span>?
            Its credentials and subfolders will be moved to My Workspace — this cannot be undone.
          </p>
          {#if error}
            <p class="text-brand-400 text-xs">{error}</p>
          {/if}
          <div class="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onclick={onClose}
              disabled={loading}
              class="px-3 py-1.5 text-xs text-gray-400 hover:text-gray-200 transition-colors"
            >Cancel</button>
            <button
              type="button"
              onclick={handleSubmit}
              disabled={loading}
              class="px-3 py-1.5 text-xs bg-brand-600 hover:bg-brand-500 disabled:opacity-50 rounded font-medium transition-colors"
            >{loading ? "Deleting..." : "Delete"}</button>
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}
