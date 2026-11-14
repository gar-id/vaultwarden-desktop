<script lang="ts">
  import { shareModalTarget } from "../lib/stores";
  import { getItemPermissions, searchShareTargets, shareItem } from "../lib/tauri";
  import { useEscapeToClose } from "../lib/useEscapeToClose";
  import type { ItemPermissionDto, SearchAroResult, ShareChangeInput } from "../lib/types";
  import RoleDropdown from "./RoleDropdown.svelte";

  const isOpenAsBoolean = {
    subscribe: (run: (v: boolean) => void) =>
      shareModalTarget.subscribe((v) => run(v !== null)),
    set: (v: boolean) => {
      if (!v) shareModalTarget.set(null);
    },
  };
  useEscapeToClose(isOpenAsBoolean as import("svelte/store").Writable<boolean>);

  let permissions = $state<ItemPermissionDto[]>([]);
  let initialPermMap = new Map<string, number>(); // "aro:aro_foreign_key" -> type
  let loadingPermissions = $state(false);
  let error = $state("");
  let saving = $state(false);

  // Search state
  let searchQuery = $state("");
  let searchResults = $state<SearchAroResult[]>([]);
  let searching = $state(false);
  let searchTimer: ReturnType<typeof setTimeout>;

  $effect(() => {
    const target = $shareModalTarget;
    error = "";
    searchQuery = "";
    searchResults = [];
    permissions = [];
    initialPermMap.clear();

    if (!target) return;

    loadingPermissions = true;
    getItemPermissions(target.modelType, target.itemId)
      .then((perms) => {
        permissions = perms;
        for (const p of perms) {
          initialPermMap.set(`${p.aro}:${p.aro_foreign_key}`, p.permission_type);
        }
      })
      .catch((e) => {
        error = `Failed to load permissions: ${e instanceof Error ? e.message : String(e)}`;
      })
      .finally(() => {
        loadingPermissions = false;
      });
  });

  function handleSearchInput() {
    clearTimeout(searchTimer);
    if (!searchQuery.trim()) {
      searchResults = [];
      searching = false;
      return;
    }
    searching = true;
    searchTimer = setTimeout(async () => {
      try {
        const results = await searchShareTargets(searchQuery.trim());
        // Filter out targets that already have access in permissions list
        const existingKeys = new Set(permissions.map((p) => `${p.aro}:${p.aro_foreign_key}`));
        searchResults = results.filter((r) => !existingKeys.has(`${r.kind}:${r.id}`));
      } catch (e) {
        console.error(e);
      } finally {
        searching = false;
      }
    }, 250);
  }

  function addCollaborator(aro: SearchAroResult) {
    permissions = [
      ...permissions,
      {
        id: null,
        aro: aro.kind,
        aro_foreign_key: aro.id,
        name: aro.label,
        identifier: aro.sublabel,
        permission_type: 1, // Default to Read-only
      },
    ];
    searchQuery = "";
    searchResults = [];
  }

  function updatePermission(permKey: string, newType: number) {
    if (newType === -1) {
      // Remove from list
      permissions = permissions.filter((p) => `${p.aro}:${p.aro_foreign_key}` !== permKey);
    } else {
      permissions = permissions.map((p) =>
        `${p.aro}:${p.aro_foreign_key}` === permKey ? { ...p, permission_type: newType } : p
      );
    }
  }

  async function handleSave() {
    const target = $shareModalTarget;
    if (!target) return;

    saving = true;
    error = "";

    try {
      const changes: ShareChangeInput[] = [];
      const currentKeys = new Set<string>();

      for (const p of permissions) {
        const key = `${p.aro}:${p.aro_foreign_key}`;
        currentKeys.add(key);
        const originalType = initialPermMap.get(key);

        if (originalType === undefined) {
          // Newly added collaborator
          changes.push({
            id: null,
            aro: p.aro,
            aro_foreign_key: p.aro_foreign_key,
            permission_type: p.permission_type,
            is_new: true,
          });
        } else if (originalType !== p.permission_type) {
          // Changed permission level
          changes.push({
            id: p.id,
            aro: p.aro,
            aro_foreign_key: p.aro_foreign_key,
            permission_type: p.permission_type,
            is_new: false,
          });
        }
      }

      // Check for removed collaborators
      for (const [key] of initialPermMap.entries()) {
        if (!currentKeys.has(key)) {
          const [aro, foreignKey] = key.split(":");
          changes.push({
            id: null,
            aro,
            aro_foreign_key: foreignKey,
            permission_type: -1,
            is_new: false,
          });
        }
      }

      if (changes.length > 0) {
        await shareItem(target.modelType, target.itemId, changes);
      }

      shareModalTarget.set(null);
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }
</script>

{#if $shareModalTarget !== null}
  <div class="fixed inset-x-0 bottom-0 top-9 bg-black/70 flex items-center justify-center z-50">
    <div
      class="bg-gray-900 border border-gray-800 rounded-lg p-6 w-[520px] max-h-[85vh] flex flex-col space-y-4 shadow-2xl text-sm"
      role="dialog"
      aria-modal="true"
    >
      <div class="flex items-start justify-between">
        <div>
          <h2 class="text-base font-semibold text-gray-100">
            Share {$shareModalTarget.modelType === "folder" ? "Folder" : "Credential"}
          </h2>
          <p class="text-xs text-gray-400 truncate mt-0.5 max-w-[420px]">
            {$shareModalTarget.name}
          </p>
        </div>
        <button
          type="button"
          onclick={() => shareModalTarget.set(null)}
          title="Close"
          aria-label="Close"
          class="text-gray-500 hover:text-gray-300 transition-colors p-1"
        >
          <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>

      <!-- Add collaborator search input -->
      <div class="relative">
        <label for="collab-search" class="text-xs text-gray-400 uppercase tracking-wide block mb-1">
          Add User or Group
        </label>
        <input
          id="collab-search"
          type="text"
          bind:value={searchQuery}
          oninput={handleSearchInput}
          placeholder="Search by name, email, or group..."
          class="w-full bg-gray-800 text-gray-100 placeholder-gray-500 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500"
        />

        {#if searching}
          <div class="absolute right-3 top-8 text-xs text-gray-500">Searching...</div>
        {/if}

        <!-- Search autocomplete results dropdown -->
        {#if searchResults.length > 0}
          <div class="absolute left-0 right-0 top-full mt-1 bg-gray-900 border border-gray-800 rounded-lg shadow-2xl max-h-48 overflow-y-auto z-10 py-1">
            {#each searchResults as r}
              <button
                type="button"
                onclick={() => addCollaborator(r)}
                class="w-full text-left px-3 py-2 hover:bg-gray-800 flex items-center justify-between text-xs transition-colors"
              >
                <div class="min-w-0 flex-1">
                  <p class="font-medium text-gray-100 truncate">{r.label}</p>
                  <p class="text-gray-500 text-[11px] truncate">{r.sublabel}</p>
                </div>
                <span class="px-2 py-0.5 rounded text-[10px] uppercase font-semibold tracking-wider ml-2 shrink-0
                             {r.kind === 'Group' ? 'bg-indigo-900/60 text-indigo-300' : 'bg-gray-800 text-gray-400'}">
                  {r.kind}
                </span>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Collaborators access list -->
      <div class="flex-1 overflow-y-auto min-h-0 space-y-2 border-t border-b border-gray-800 py-3">
        <p class="text-xs text-gray-400 uppercase tracking-wide">Who has access</p>

        {#if loadingPermissions}
          <p class="text-xs text-gray-500 py-4 text-center">Loading collaborators...</p>
        {:else if permissions.length === 0}
          <p class="text-xs text-gray-500 py-4 text-center">No explicit permissions found.</p>
        {:else}
          <div class="space-y-1.5">
            {#each permissions as perm}
              {@const permKey = `${perm.aro}:${perm.aro_foreign_key}`}
              <div class="flex items-center justify-between gap-3 bg-gray-950/60 border border-gray-800/80 rounded px-3 py-2 text-xs">
                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-2">
                    <span class="font-medium text-gray-100 truncate">{perm.name}</span>
                    <span class="text-[10px] uppercase tracking-wider text-gray-500 font-semibold">({perm.aro})</span>
                  </div>
                  {#if perm.identifier && perm.identifier !== perm.name}
                    <p class="text-[11px] text-gray-500 truncate">{perm.identifier}</p>
                  {/if}
                </div>

                <RoleDropdown
                  value={perm.permission_type}
                  onChange={(newType) => updatePermission(permKey, newType)}
                />
              </div>
            {/each}
          </div>
        {/if}
      </div>

      {#if error}
        <p class="text-brand-400 text-xs">{error}</p>
      {/if}

      <div class="flex justify-end gap-2 pt-1">
        <button
          type="button"
          onclick={() => shareModalTarget.set(null)}
          disabled={saving}
          class="px-4 py-1.5 text-xs text-gray-400 hover:text-gray-200 transition-colors"
        >
          Cancel
        </button>
        <button
          type="button"
          onclick={handleSave}
          disabled={saving || loadingPermissions}
          class="px-4 py-1.5 text-xs bg-brand-600 hover:bg-brand-500 disabled:opacity-50 text-white rounded font-medium transition-colors"
        >
          {saving ? "Sharing..." : "Save Changes"}
        </button>
      </div>
    </div>
  </div>
{/if}
