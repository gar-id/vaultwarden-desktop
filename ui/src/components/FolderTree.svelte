<script lang="ts">
  import { onMount } from "svelte";
  import { listFolderTree, moveItem, listVaultTags } from "../lib/tauri";
  import { folderTree, selectedFolderId, resources, shareModalTarget, refreshVaultItems } from "../lib/stores";
  import type { FolderNode } from "../lib/types";
  import FolderTreeNode from "./FolderTreeNode.svelte";
  import FolderContextMenu from "./FolderContextMenu.svelte";
  import FolderModal, { type FolderModalState } from "./FolderModal.svelte";

  let contextMenu = $state<{ node: FolderNode; x: number; y: number } | null>(null);
  let modalState = $state<FolderModalState>(null);
  let rootDragOver = $state(false);
  let tags = $state<string[]>([]);

  async function loadData() {
    try {
      folderTree.set(await listFolderTree());
      tags = await listVaultTags();
    } catch (e) {
      console.error("FolderTree: loadData failed:", e);
    }
  }

  onMount(() => {
    loadData();
  });

  // Re-fetch tags whenever resources change (e.g. sync or edit)
  $effect(() => {
    const _ = $resources;
    listVaultTags().then((t) => (tags = t)).catch(() => {});
  });

  function selectAll() {
    selectedFolderId.set(null);
  }

  function handleContextMenu(e: MouseEvent, node: FolderNode) {
    contextMenu = { node, x: e.clientX, y: e.clientY };
  }

  function handleContextAction(action: "create-child" | "rename" | "delete" | "share", node: FolderNode) {
    if (action === "create-child") {
      modalState = { action: "create", parentId: node.id, parentName: node.name };
    } else if (action === "rename") {
      modalState = { action: "rename", folderId: node.id, currentName: node.name };
    } else if (action === "delete") {
      modalState = { action: "delete", folderId: node.id, folderName: node.name };
    } else if (action === "share") {
      shareModalTarget.set({
        modelType: "folder",
        itemId: node.id,
        name: node.name,
      });
    }
  }

  async function handleDropItem(modelType: "folder" | "resource", itemId: string, targetFolderId: string | null) {
    try {
      await moveItem(modelType, itemId, targetFolderId);
      // Refresh folder tree (which recalculates resource_count and hierarchy)
      folderTree.set(await listFolderTree());

      // Refresh current secret list
      await refreshVaultItems();
    } catch (e) {
      console.error(`Failed to move ${modelType} ${itemId}:`, e);
    }
  }

  function handleRootDragOver(e: DragEvent) {
    e.preventDefault();
    if (!e.dataTransfer) return;
    e.dataTransfer.dropEffect = "move";
    rootDragOver = true;
  }

  function handleRootDragLeave() {
    rootDragOver = false;
  }

  async function handleRootDrop(e: DragEvent) {
    e.preventDefault();
    rootDragOver = false;
    if (!e.dataTransfer) return;
    const raw = e.dataTransfer.getData("application/json");
    if (!raw) return;
    try {
      const data = JSON.parse(raw);
      await handleDropItem(data.type, data.id, null);
    } catch (err) {
      console.error("Root drop error:", err);
    }
  }
</script>

<!-- Column 1: folder tree. min-h-0 on this flex child + overflow-y-auto on
     the scrollable list below is what actually lets the list scroll
     independently of the page - without min-h-0, a flex child defaults to
     a min-height equal to its content, so a tall tree just grows the
     whole column instead of scrolling inside it. -->
<section class="flex flex-col w-56 bg-gray-900 border-r border-gray-800 shrink-0 min-h-0">
  <div class="px-3 py-2.5 border-b border-gray-800 shrink-0 flex items-center justify-between">
    <h2 class="text-xs font-semibold text-gray-500 uppercase tracking-wide">Folders</h2>
    <button
      onclick={() => (modalState = { action: "create" })}
      title="New Folder"
      class="w-6 h-6 flex items-center justify-center rounded bg-brand-600 hover:bg-brand-500 text-white transition-colors text-sm font-semibold shadow-sm"
    >+</button>
  </div>

  <nav class="flex-1 overflow-y-auto min-h-0 py-1">
    <button
      onclick={selectAll}
      ondragover={handleRootDragOver}
      ondragleave={handleRootDragLeave}
      ondrop={handleRootDrop}
      class="w-full text-left px-3 py-1.5 text-sm transition-colors
             {rootDragOver ? 'bg-brand-900/60 ring-1 ring-brand-500' : $selectedFolderId === null ? 'bg-gray-800 text-gray-100 font-medium' : 'text-gray-400 hover:bg-gray-800/50 hover:text-gray-200'}"
    >
      My Workspace
    </button>

    <button
      onclick={() => selectedFolderId.set("__favorites__")}
      class="w-full text-left px-3 py-1.5 text-sm transition-colors flex items-center gap-2
             {$selectedFolderId === '__favorites__' ? 'bg-gray-800 text-brand-400 font-medium' : 'text-gray-400 hover:bg-gray-800/50 hover:text-gray-200'}"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5 fill-current text-brand-500" viewBox="0 0 24 24">
        <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
      </svg>
      <span>Favorites</span>
    </button>

    {#if $folderTree.length > 0}
      <ul class="mt-0.5">
        {#each $folderTree as node}
          <FolderTreeNode {node} depth={0} onContextMenu={handleContextMenu} onDropItem={handleDropItem} />
        {/each}
      </ul>
    {/if}

    {#if tags.length > 0}
      <div class="mt-4 pt-2 border-t border-gray-800 px-3">
        <h3 class="text-[11px] font-semibold text-gray-500 uppercase tracking-wider mb-1.5">Tags</h3>
        <ul class="space-y-0.5">
          {#each tags as tag}
            {@const tagKey = `__tag:${tag}__`}
            <li>
              <button
                onclick={() => selectedFolderId.set(tagKey)}
                class="w-full text-left px-2 py-1 text-xs rounded transition-colors flex items-center gap-1.5 truncate
                       {$selectedFolderId === tagKey ? 'bg-gray-800 text-brand-400 font-medium' : 'text-gray-400 hover:bg-gray-800/50 hover:text-gray-200'}"
              >
                <span class="text-brand-500 font-mono text-[11px]">#</span>
                <span class="truncate">{tag}</span>
              </button>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  </nav>

  {#if contextMenu}
    {@const activeNode = contextMenu.node}
    {@const activeX = contextMenu.x}
    {@const activeY = contextMenu.y}
    <FolderContextMenu
      node={activeNode}
      x={activeX}
      y={activeY}
      onClose={() => (contextMenu = null)}
      onAction={handleContextAction}
    />
  {/if}

  <FolderModal modal={modalState} onClose={() => (modalState = null)} />
</section>
