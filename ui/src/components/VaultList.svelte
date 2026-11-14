<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { searchResources, deleteResource } from "../lib/tauri";
  import {
    resources,
    selectedResource,
    showPasswordGen,
    selectedFolderId,
    searchQuery,
    refreshVaultItems,
    resourceFormMode,
    shareModalTarget,
  } from "../lib/stores";
  import type { ResourceItem } from "../lib/types";
  import VaultItemContextMenu from "./VaultItemContextMenu.svelte";

  let debounceTimer: ReturnType<typeof setTimeout>;
  let loadError = $state("");

  let contextMenu = $state<{ item: ResourceItem; x: number; y: number } | null>(null);
  let confirmingDeleteItem = $state<ResourceItem | null>(null);
  let deletingItem = $state(false);
  let deleteItemError = $state("");

  async function runSearch(q: string, folderId: string | null) {
    try {
      const results = await searchResources(q, folderId);
      resources.set(results);
      loadError = "";
    } catch (e) {
      // Surface this in the UI, not just the console - a silently empty
      // list and a silently failed search look identical to a user
      // unless the failure is shown somewhere they'll actually see it.
      loadError = e instanceof Error ? e.message : String(e);
      console.error("VaultList: search_resources failed:", e);
    }
  }

  $effect(() => {
    // track query AND the selected folder - either changing re-runs search
    const q = $searchQuery;
    const folderId = $selectedFolderId;
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => runSearch(q, folderId), 150);
  });

  onMount(() => {
    runSearch($searchQuery, get(selectedFolderId));
  });

  function select(r: ResourceItem) {
    selectedResource.set(r);
  }

  function handleContextMenu(e: MouseEvent, r: ResourceItem) {
    e.preventDefault();
    selectedResource.set(r);
    contextMenu = { item: r, x: e.clientX, y: e.clientY };
  }

  function handleContextAction(action: "edit" | "clone" | "delete" | "share", item: ResourceItem) {
    if (action === "edit") {
      resourceFormMode.set(item);
    } else if (action === "clone") {
      resourceFormMode.set({ cloneFrom: item });
    } else if (action === "delete") {
      confirmingDeleteItem = item;
      deleteItemError = "";
    } else if (action === "share") {
      shareModalTarget.set({
        modelType: "resource",
        itemId: item.id,
        name: item.name,
      });
    }
  }

  async function handleDeleteConfirm() {
    if (!confirmingDeleteItem) return;
    deletingItem = true;
    deleteItemError = "";
    try {
      await deleteResource(confirmingDeleteItem.id);
      await refreshVaultItems();
      if (get(selectedResource)?.id === confirmingDeleteItem.id) {
        selectedResource.set(null);
      }
      confirmingDeleteItem = null;
    } catch (e: unknown) {
      deleteItemError = e instanceof Error ? e.message : String(e);
    } finally {
      deletingItem = false;
    }
  }
</script>

<!-- Column 2: the secret list. min-h-0 lets the <ul> below scroll on its
     own instead of growing the whole column (same fix as FolderTree). -->
<section class="flex flex-col w-72 bg-gray-900 border-r border-gray-800 shrink-0 min-h-0">
  <div class="p-3 border-b border-gray-800 shrink-0 flex gap-2">
    <input
      bind:value={$searchQuery}
      placeholder="Search vault..."
      class="flex-1 bg-gray-800 text-gray-100 placeholder-gray-500 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500"
    />
    <button
      onclick={() => resourceFormMode.set("create")}
      title="New credential"
      class="w-8 h-8 shrink-0 rounded bg-brand-600 hover:bg-brand-500 flex items-center justify-center text-lg font-medium transition-colors"
    >+</button>
  </div>

  {#if loadError}
    <p class="px-3 py-2 text-xs text-red-400 border-b border-gray-800 shrink-0">{loadError}</p>
  {/if}

  <ul class="flex-1 overflow-y-auto min-h-0">
    {#each $resources as r}
      <li>
        <button
          draggable="true"
          ondragstart={(e) => {
            if (!e.dataTransfer) return;
            e.dataTransfer.setData("application/json", JSON.stringify({ type: "resource", id: r.id }));
            e.dataTransfer.effectAllowed = "move";
          }}
          onclick={() => select(r)}
          oncontextmenu={(e) => handleContextMenu(e, r)}
          class="w-full text-left px-3 py-2.5 hover:bg-gray-800 border-b border-gray-800/50 transition-colors flex items-center justify-between gap-2
                 {$selectedResource?.id === r.id ? 'bg-gray-800' : ''}"
        >
          <div class="min-w-0 flex-1">
            <p class="font-medium text-gray-100 truncate">{r.name}</p>
            <p class="text-xs text-gray-500 truncate">{r.username ?? r.uri ?? ""}</p>
            {#if r.tags && r.tags.length > 0}
              <div class="flex items-center gap-1 mt-1 flex-wrap">
                {#each r.tags.slice(0, 3) as tag}
                  <span class="text-[10px] text-gray-400 bg-gray-800 border border-gray-700/60 rounded px-1 py-0.2">#{tag}</span>
                {/each}
                {#if r.tags.length > 3}
                  <span class="text-[10px] text-gray-500">+{r.tags.length - 3}</span>
                {/if}
              </div>
            {/if}
          </div>
          {#if r.is_favorite}
            <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5 fill-current text-brand-500 shrink-0" viewBox="0 0 24 24">
              <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
            </svg>
          {/if}
        </button>
      </li>
    {/each}
    {#if $resources.length === 0 && !loadError}
      <li class="px-4 py-8 text-center text-gray-600 text-xs">No credentials found</li>
    {/if}
  </ul>

  <div class="p-3 border-t border-gray-800 shrink-0">
    <button
      onclick={() => showPasswordGen.set(true)}
      class="w-full text-xs text-gray-400 hover:text-brand-400 transition-colors text-left"
    >Generate password</button>
  </div>

  {#if contextMenu}
    {@const activeItem = contextMenu.item}
    {@const activeX = contextMenu.x}
    {@const activeY = contextMenu.y}
    <VaultItemContextMenu
      item={activeItem}
      x={activeX}
      y={activeY}
      onClose={() => (contextMenu = null)}
      onAction={handleContextAction}
    />
  {/if}

  {#if confirmingDeleteItem}
    <div class="fixed inset-x-0 bottom-0 top-9 bg-black/70 flex items-center justify-center z-50">
      <div class="bg-gray-900 border border-gray-800 rounded-lg p-5 w-80 space-y-3 shadow-2xl text-sm" role="dialog" aria-modal="true">
        <h2 class="text-base font-semibold text-gray-100">Delete Credential</h2>
        <p class="text-gray-300 text-xs leading-relaxed">
          Delete <span class="font-semibold text-gray-100">"{confirmingDeleteItem.name}"</span>? This removes it from the server for everyone with access — it cannot be undone.
        </p>
        {#if deleteItemError}
          <p class="text-brand-400 text-xs">{deleteItemError}</p>
        {/if}
        <div class="flex justify-end gap-2 pt-1">
          <button
            onclick={() => (confirmingDeleteItem = null)}
            disabled={deletingItem}
            class="px-3 py-1.5 text-xs text-gray-400 hover:text-gray-200 transition-colors"
          >Cancel</button>
          <button
            onclick={handleDeleteConfirm}
            disabled={deletingItem}
            class="px-3 py-1.5 text-xs bg-brand-600 hover:bg-brand-500 disabled:opacity-50 rounded font-medium transition-colors"
          >{deletingItem ? "Deleting..." : "Delete"}</button>
        </div>
      </div>
    </div>
  {/if}
</section>
