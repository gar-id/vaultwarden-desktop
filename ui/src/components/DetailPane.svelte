<script lang="ts">
  import {
    selectedResource,
    resourceFormMode,
    selectedFolderId,
    folderTree,
    shareModalTarget,
    refreshVaultItems,
  } from "../lib/stores";
  import {
    copySecret,
    revealSecret,
    getTotpCode,
    deleteResource,
    toggleFavorite,
    getResourceComments,
    addResourceComment,
    deleteResourceComment,
    getResourceActivity,
  } from "../lib/tauri";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { RevealedSecret, FolderNode, ResourceComment, ResourceActivity } from "../lib/types";
  import CopyButton from "./CopyButton.svelte";

  let passwordRevealed = $state(false);
  let noteRevealed = $state(false);
  let loadingSecret = $state(false);
  let revealedSecret = $state<RevealedSecret | null>(null);
  let copyTimer = $state(0);
  let copyInterval: ReturnType<typeof setInterval>;

  let confirmingDelete = $state(false);
  let deleting = $state(false);
  let deleteError = $state("");

  // TOTP is shown whenever the resource has one, independent of the
  // password reveal toggle — a user with a TOTP-only resource has no
  // password to reveal at all, so this can't be gated behind `revealed`.
  let totpCode = $state("");
  let totpExpiresIn = $state(0);
  let totpInterval: ReturnType<typeof setInterval>;
  let hasTotp = $state(false);

  let comments = $state<ResourceComment[]>([]);
  let loadingComments = $state(false);
  let newCommentText = $state("");
  let postingComment = $state(false);
  let commentError = $state("");

  let activeTab = $state<"password_history" | "activity">("password_history");
  let revealedFields = $state<Record<number, boolean>>({});
  let revealedHistory = $state<Record<number, boolean>>({});
  let activities = $state<ResourceActivity[]>([]);
  let loadingActivity = $state(false);

  async function loadActivity(resourceId: string) {
    loadingActivity = true;
    try {
      activities = await getResourceActivity(resourceId);
    } catch (_e) {
      activities = [];
    } finally {
      loadingActivity = false;
    }
  }

  function formatActionLabel(type: string | null | undefined, context: string | null | undefined): string {
    if (!type && !context) return "Activity event";
    const t = (type || "").toLowerCase();
    if (t.includes("resources.add") || t.includes("resource.add")) return "Created credential";
    if (t.includes("resources.view") || t.includes("resource.view")) return "Viewed credential";
    if (t.includes("resources.edit") || t.includes("resources.update") || t.includes("resource.update")) return "Updated credential details";
    if (t.includes("secrets.view") || t.includes("secret.view")) return "Decrypted password / secret";
    if (t.includes("secrets.edit") || t.includes("secrets.update") || t.includes("secret.update")) return "Updated password / secret revision";
    if (t.includes("permissions.add") || t.includes("share")) return "Changed sharing permissions";
    if (t.includes("comments.add") || t.includes("comment.add")) return "Added comment";
    if (type) return type;
    return context || "Activity event";
  }

  async function loadComments(resourceId: string) {
    loadingComments = true;
    try {
      comments = await getResourceComments(resourceId);
    } catch (_e) {
      comments = [];
    } finally {
      loadingComments = false;
    }
  }

  async function handleAddComment() {
    if (!newCommentText.trim() || !$selectedResource) return;
    postingComment = true;
    commentError = "";
    try {
      const added = await addResourceComment($selectedResource.id, newCommentText.trim());
      comments = [...comments, added];
      newCommentText = "";
    } catch (e: unknown) {
      commentError = e instanceof Error ? e.message : String(e);
    } finally {
      postingComment = false;
    }
  }

  async function handleDeleteComment(commentId: string) {
    try {
      await deleteResourceComment(commentId);
      comments = comments.filter((c) => c.id !== commentId);
    } catch (e: unknown) {
      console.error(e);
    }
  }

  let allRevealed = $derived(passwordRevealed || noteRevealed);
  let togglingFavorite = $state(false);

  function findFolderHierarchy(nodes: FolderNode[], targetId: string | null): string[] {
    if (!targetId) return [];
    for (const node of nodes) {
      if (node.id === targetId) {
        return [node.name];
      }
      if (node.children && node.children.length > 0) {
        const sub = findFolderHierarchy(node.children, targetId);
        if (sub.length > 0) {
          return [node.name, ...sub];
        }
      }
    }
    return [];
  }

  let breadcrumbs = $derived.by(() => {
    if (!$selectedResource) return [];
    const folders = findFolderHierarchy($folderTree, $selectedResource.folder_parent_id);
    if (folders.length === 0) {
      return ["My Workspace", $selectedResource.name];
    }
    return [...folders, $selectedResource.name];
  });

  function formatTypeBadge(slug: string | null | undefined): string {
    if (!slug) return "Password";
    if (slug.includes("totp") && (slug.includes("password") || slug.includes("default"))) {
      return "Password + TOTP";
    }
    if (slug.includes("totp")) {
      return "TOTP";
    }
    return "Password";
  }

  async function handleToggleFavorite() {
    if (!$selectedResource || togglingFavorite) return;
    togglingFavorite = true;
    try {
      const isFav = await toggleFavorite($selectedResource.id);
      $selectedResource.is_favorite = isFav;
      resources.update((list) =>
        list.map((item) =>
          item.id === $selectedResource.id ? { ...item, is_favorite: isFav } : item
        )
      );
    } catch (e) {
      console.error("Failed to toggle favorite:", e);
    } finally {
      togglingFavorite = false;
    }
  }

  async function ensureSecretLoaded() {
    if (!$selectedResource) return;
    if (!revealedSecret) {
      revealedSecret = await revealSecret($selectedResource.id);
    }
  }

  async function refreshTotp() {
    if (!$selectedResource) return;
    try {
      const result = await getTotpCode($selectedResource.id);
      totpCode = result.code;
      totpExpiresIn = result.expires_in_secs;
    } catch (e) {
      console.error(e);
    }
  }

  async function handleCopy() {
    if (!$selectedResource) return;
    await copySecret($selectedResource.id);
    copyTimer = 30;
    clearInterval(copyInterval);
    copyInterval = setInterval(() => {
      copyTimer--;
      if (copyTimer <= 0) clearInterval(copyInterval);
    }, 1000);
  }

  async function handleTogglePassword() {
    if (!$selectedResource) return;
    if (!passwordRevealed) {
      await ensureSecretLoaded();
      passwordRevealed = true;
    } else {
      passwordRevealed = false;
    }
  }

  async function handleToggleNote() {
    if (!$selectedResource) return;
    if (!noteRevealed) {
      await ensureSecretLoaded();
      noteRevealed = true;
    } else {
      noteRevealed = false;
    }
  }

  async function handleToggleRevealAll() {
    if (!$selectedResource) return;
    if (allRevealed) {
      passwordRevealed = false;
      noteRevealed = false;
      revealedFields = {};
      revealedHistory = {};
    } else {
      await ensureSecretLoaded();
      passwordRevealed = true;
      noteRevealed = true;
      if (revealedSecret?.fields) {
        revealedSecret.fields.forEach((_, idx) => {
          revealedFields[idx] = true;
        });
      }
      if (revealedSecret?.password_history) {
        revealedSecret.password_history.forEach((_, idx) => {
          revealedHistory[idx] = true;
        });
      }
    }
  }

  function handleEdit() {
    if ($selectedResource) resourceFormMode.set($selectedResource);
  }

  function handleOpenUri(e: MouseEvent, rawUri: string) {
    e.preventDefault();
    const url = /^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//.test(rawUri) ? rawUri : `http://${rawUri}`;
    openUrl(url).catch((err) => {
      console.error("Failed to open URL:", err);
    });
  }

  async function handleConfirmDelete() {
    if (!$selectedResource) return;
    deleting = true;
    deleteError = "";
    try {
      await deleteResource($selectedResource.id);
      await refreshVaultItems();
      selectedResource.set(null);
      confirmingDelete = false;
    } catch (e: unknown) {
      deleteError = e instanceof Error ? e.message : String(e);
    } finally {
      deleting = false;
    }
  }

  $effect(() => {
    // Reset reveal + TOTP polling + delete confirmation whenever the
    // selected resource changes - confirmingDelete must never carry over
    // from one resource to the next.
    const r = $selectedResource;
    passwordRevealed = false;
    noteRevealed = false;
    revealedSecret = null;
    clearInterval(copyInterval);
    copyTimer = 0;
    clearInterval(totpInterval);
    totpCode = "";
    totpExpiresIn = 0;
    hasTotp = false;
    confirmingDelete = false;
    deleteError = "";
    comments = [];
    newCommentText = "";
    commentError = "";
    activities = [];
    revealedFields = {};
    revealedHistory = {};
    activeTab = "password_history";
    loadingSecret = true;

    if (!r) {
      loadingSecret = false;
      return;
    }

    loadComments(r.id);
    loadActivity(r.id);

    // Check TOTP capability and preload secret into memory for fast reveal
    (async () => {
      try {
        const secret = await revealSecret(r.id);
        revealedSecret = secret;
        if (secret.has_totp) {
          hasTotp = true;
          await refreshTotp();
          totpInterval = setInterval(refreshTotp, 1000);
        }
      } catch (e) {
        console.error(e);
      } finally {
        loadingSecret = false;
      }
    })();

    return () => clearInterval(totpInterval);
  });
</script>

<main class="flex-1 p-6 overflow-y-auto min-h-0">
  {#if !$selectedResource}
    <div class="flex items-center justify-center h-full text-gray-600">
      Select a credential to view details
    </div>
  {:else}
    <div class="flex items-start justify-between gap-4 mb-6">
      <div class="min-w-0">
        <div class="flex items-center gap-2">
          <h2 class="text-xl font-semibold text-gray-100 truncate">{$selectedResource.name}</h2>
          <button
            type="button"
            onclick={handleToggleFavorite}
            disabled={togglingFavorite}
            title={$selectedResource.is_favorite ? "Remove from favorites" : "Mark as favorite"}
            class="p-1 rounded text-gray-500 hover:text-brand-400 hover:bg-gray-800 transition-colors shrink-0"
          >
            {#if $selectedResource.is_favorite}
              <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5 fill-current text-brand-500" viewBox="0 0 24 24">
                <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
              </svg>
            {:else}
              <svg xmlns="http://www.w3.org/2000/svg" class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
              </svg>
            {/if}
          </button>
        </div>

        {#if breadcrumbs.length > 0}
          <div class="flex items-center gap-1.5 text-xs text-gray-500 font-medium truncate mt-1">
            {#each breadcrumbs as segment, idx}
              {#if idx > 0}
                <span class="text-gray-600 select-none">/</span>
              {/if}
              <span class={idx === breadcrumbs.length - 1 ? 'text-gray-300 font-semibold truncate' : 'truncate'}>
                {segment}
              </span>
            {/each}
          </div>
        {/if}

        <div class="mt-2 flex items-center gap-1.5 flex-wrap">
          <span class="px-2 py-0.5 rounded text-[10px] font-semibold tracking-wider uppercase bg-gray-800 text-gray-300 border border-gray-700/60 inline-block">
            {formatTypeBadge($selectedResource.resource_type_slug)}
          </span>
          {#if $selectedResource.tags && $selectedResource.tags.length > 0}
            {#each $selectedResource.tags as tag}
              <span class="px-2 py-0.5 rounded text-[10px] font-medium bg-gray-800 text-gray-400 border border-gray-700/60 inline-block">
                #{tag}
              </span>
            {/each}
          {/if}
        </div>
      </div>
      <div class="flex gap-2 shrink-0">
        <button
          onclick={handleToggleRevealAll}
          class="px-3.5 py-1.5 text-xs bg-brand-600 hover:bg-brand-500 text-white rounded font-medium transition-colors flex items-center gap-1.5 shadow-sm"
          title={allRevealed ? "Hide all sensitive fields" : "Reveal all sensitive fields"}
        >
          {#if allRevealed}
            <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
              <line x1="1" y1="1" x2="23" y2="23" />
            </svg>
            Hide All
          {:else}
            <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
            Reveal All
          {/if}
        </button>
        <button onclick={handleEdit}
          class="px-3 py-1.5 text-xs text-gray-400 hover:text-gray-200 border border-gray-700 hover:border-gray-600 rounded transition-colors">
          Edit
        </button>
        <button
          onclick={() => {
            if ($selectedResource) {
              shareModalTarget.set({
                modelType: "resource",
                itemId: $selectedResource.id,
                name: $selectedResource.name,
              });
            }
          }}
          class="px-3 py-1.5 text-xs text-gray-400 hover:text-gray-200 border border-gray-700 hover:border-gray-600 rounded transition-colors flex items-center gap-1.5"
        >
          <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="18" cy="5" r="3" />
            <circle cx="6" cy="12" r="3" />
            <circle cx="18" cy="19" r="3" />
            <line x1="8.59" y1="13.51" x2="15.42" y2="17.49" />
            <line x1="15.41" y1="6.51" x2="8.59" y2="10.49" />
          </svg>
          Share
        </button>
        <button onclick={() => (confirmingDelete = true)}
          class="px-3 py-1.5 text-xs text-red-400 hover:text-red-300 border border-gray-700 hover:border-red-800 rounded transition-colors">
          Delete
        </button>
      </div>
    </div>

    {#if confirmingDelete}
      <div class="mb-6 p-3 bg-gray-900 border border-red-900/50 rounded space-y-2">
        <p class="text-gray-300 text-xs">
          Delete "{$selectedResource.name}"? This removes it from the server for everyone with access — it cannot be undone.
        </p>
        {#if deleteError}
          <p class="text-red-400 text-xs">{deleteError}</p>
        {/if}
        <div class="flex justify-end gap-2">
          <button
            onclick={() => (confirmingDelete = false)}
            disabled={deleting}
            class="px-2 py-1 text-xs text-gray-400 hover:text-gray-200 transition-colors"
          >Cancel</button>
          <button
            onclick={handleConfirmDelete}
            disabled={deleting}
            class="px-2 py-1 text-xs bg-red-600 hover:bg-red-500 disabled:opacity-50 rounded font-medium transition-colors"
          >{deleting ? "Deleting..." : "Delete"}</button>
        </div>
      </div>
    {/if}

    <div class="space-y-3">
      {#if $selectedResource.username}
        <div class="bg-gray-900 border border-gray-800 rounded-lg px-4 py-3 flex items-start justify-between gap-2">
          <div class="min-w-0 flex-1">
            <p class="text-xs text-gray-500 uppercase tracking-wide mb-1">Username</p>
            <p class="font-mono text-gray-100 truncate">{$selectedResource.username}</p>
          </div>
          <CopyButton value={$selectedResource.username} title="Copy username" />
        </div>
      {/if}

      {#if $selectedResource.uri}
        <div class="bg-gray-900 border border-gray-800 rounded-lg px-4 py-3 flex items-start justify-between gap-2">
          <div class="min-w-0 flex-1">
            <p class="text-xs text-gray-500 uppercase tracking-wide mb-1">URL / Host</p>
            <a
              href={$selectedResource.uri}
              onclick={(e) => handleOpenUri(e, $selectedResource.uri!)}
              class="text-brand-400 hover:underline break-all block cursor-pointer"
              title="Open {$selectedResource.uri}"
            >
              {$selectedResource.uri}
            </a>
          </div>
          <CopyButton value={$selectedResource.uri} title="Copy URL/Host" />
        </div>
      {/if}

      {#if !hasTotp || revealedSecret?.password}
        <div class="bg-gray-900 border border-gray-800 rounded-lg px-4 py-3 flex items-start justify-between gap-2">
          <div class="min-w-0 flex-1">
            <div class="flex items-center gap-2 mb-1">
              <p class="text-xs text-gray-500 uppercase tracking-wide">Password</p>
              {#if revealedSecret?.password_history && revealedSecret.password_history.length > 0}
                <button
                  type="button"
                  onclick={() => (activeTab = "password_history")}
                  class="text-[10px] text-brand-400 hover:text-brand-300 font-mono bg-brand-950/80 px-1.5 py-0.5 rounded border border-brand-800/60 transition-colors flex items-center gap-1 cursor-pointer"
                  title="View {revealedSecret.password_history.length} previous passwords in history"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" class="w-2.5 h-2.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <circle cx="12" cy="12" r="10"></circle>
                    <polyline points="12 6 12 12 16 14"></polyline>
                  </svg>
                  <span>History ({revealedSecret.password_history.length})</span>
                </button>
              {/if}
            </div>
            <p class="font-mono text-gray-100 select-all truncate">
              {passwordRevealed ? (revealedSecret?.password ?? "(none)") : "••••••••••••"}
            </p>
          </div>
          <div class="flex items-center gap-1 shrink-0">
            <button
              type="button"
              onclick={handleTogglePassword}
              title={passwordRevealed ? "Hide password" : "Reveal password"}
              class="text-gray-400 hover:text-brand-400 transition-colors p-1 rounded hover:bg-gray-800 flex items-center justify-center w-7 h-7"
            >
              {#if passwordRevealed}
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
            <CopyButton onCopy={handleCopy} title={copyTimer > 0 ? `Copied (${copyTimer}s)` : "Copy password"} />
          </div>
        </div>
      {/if}

      {#if hasTotp}
        <div class="bg-gray-900 border border-gray-800 rounded-lg px-4 py-3 flex items-start justify-between gap-2">
          <div class="min-w-0 flex-1">
            <p class="text-xs text-gray-500 uppercase tracking-wide mb-1">One-Time Code</p>
            <div class="flex items-center gap-3">
              <span class="font-mono text-2xl text-gray-100 tracking-widest">
                {totpCode ? totpCode.replace(/(.{3})/, "$1 ") : "······"}
              </span>
              <span class="text-xs text-gray-500">{totpExpiresIn}s</span>
            </div>
          </div>
          <CopyButton value={totpCode} title="Copy code" />
        </div>
      {/if}

      <!-- Custom Fields Section -->
      {#if revealedSecret?.fields && revealedSecret.fields.length > 0}
        <div class="space-y-2">
          <p class="text-xs text-gray-500 uppercase tracking-wide px-1">Custom Fields</p>
          {#each revealedSecret.fields as field, idx}
            <div class="bg-gray-900 border border-gray-800 rounded-lg px-4 py-3 flex items-start justify-between gap-2">
              <div class="min-w-0 flex-1">
                <p class="text-xs text-gray-500 uppercase tracking-wide mb-1">{field.name || `Field #${idx + 1}`}</p>
                {#if field.type === 1}
                  <p class="font-mono text-gray-100 select-all truncate">
                    {revealedFields[idx] ? (field.value || "(empty)") : "••••••••••••"}
                  </p>
                {:else if field.type === 2}
                  <p class="text-sm font-medium {field.value === 'true' || field.value === '1' ? 'text-emerald-400' : 'text-gray-400'}">
                    {field.value === 'true' || field.value === '1' ? '✓ True / Yes' : '✕ False / No'}
                  </p>
                {:else}
                  <p class="font-mono text-gray-100 select-all truncate">{field.value || "(empty)"}</p>
                {/if}
              </div>
              <div class="flex items-center gap-1 shrink-0">
                {#if field.type === 1}
                  <button
                    type="button"
                    onclick={() => (revealedFields[idx] = !revealedFields[idx])}
                    title={revealedFields[idx] ? "Hide field" : "Reveal field"}
                    class="text-gray-400 hover:text-brand-400 transition-colors p-1 rounded hover:bg-gray-800 flex items-center justify-center w-7 h-7 cursor-pointer"
                  >
                    {#if revealedFields[idx]}
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
                {/if}
                {#if field.type !== 2}
                  <CopyButton value={field.value ?? ""} title="Copy field value" />
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Always show Note / Description container box -->
      <div class="bg-gray-900 border border-gray-800 rounded-lg px-4 py-3 flex items-start justify-between gap-2">
        <div class="min-w-0 flex-1">
          <p class="text-xs text-gray-500 uppercase tracking-wide mb-1">Note / Description</p>
          {#if loadingSecret}
            <p class="text-xs text-gray-500 py-0.5 italic">Loading note...</p>
          {:else if !revealedSecret?.description}
            <p class="text-gray-500 text-sm italic font-sans">(none)</p>
          {:else if noteRevealed}
            <p class="text-gray-200 whitespace-pre-wrap text-sm break-words font-sans leading-relaxed select-text">
              {revealedSecret.description}
            </p>
          {:else}
            <span class="font-mono text-gray-400 select-none">••••••••••••</span>
          {/if}
        </div>
        {#if revealedSecret?.description}
          <div class="flex items-center gap-1 shrink-0">
            <button
              type="button"
              onclick={handleToggleNote}
              title={noteRevealed ? "Hide note/description" : "Reveal note/description"}
              class="text-gray-400 hover:text-brand-400 transition-colors p-1 rounded hover:bg-gray-800 flex items-center justify-center w-7 h-7 cursor-pointer"
            >
              {#if noteRevealed}
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
            <CopyButton value={revealedSecret.description} title="Copy note/description" />
          </div>
        {/if}
      </div>

      <!-- Tabbed container: Password History & Revision Info -->
      <div class="bg-gray-900 border border-gray-800 rounded-lg p-4 space-y-3">
        <!-- Tab Headers -->
        <div class="flex items-center gap-4 border-b border-gray-800 pb-2">
          <button
            type="button"
            onclick={() => (activeTab = "password_history")}
            class="text-xs uppercase tracking-wide font-semibold pb-1 -mb-2 border-b-2 transition-colors flex items-center gap-1.5 cursor-pointer
                   {activeTab === 'password_history' ? 'border-brand-500 text-gray-100' : 'border-transparent text-gray-500 hover:text-gray-300'}"
          >
            <span>Password History</span>
            {#if revealedSecret?.password_history && revealedSecret.password_history.length > 0}
              <span class="bg-brand-900/80 text-brand-300 text-[10px] px-1.5 py-0.5 rounded-full font-mono">{revealedSecret.password_history.length}</span>
            {/if}
          </button>

          <button
            type="button"
            onclick={() => (activeTab = "activity")}
            class="text-xs uppercase tracking-wide font-semibold pb-1 -mb-2 border-b-2 transition-colors flex items-center gap-1.5 cursor-pointer
                   {activeTab === 'activity' ? 'border-brand-500 text-gray-100' : 'border-transparent text-gray-500 hover:text-gray-300'}"
          >
            <span>Revision Info</span>
          </button>
        </div>

        {#if activeTab === "password_history"}
          {#if revealedSecret?.password_history && revealedSecret.password_history.length > 0}
            <div class="space-y-2 max-h-56 overflow-y-auto pr-1">
              {#each revealedSecret.password_history as hist, idx}
                <div class="bg-gray-950/70 border border-gray-800/80 rounded p-2.5 text-xs flex items-center justify-between gap-3">
                  <div class="min-w-0 flex-1 space-y-1">
                    <span class="text-gray-400 font-mono text-[11px] block">
                      {hist.last_used_date ? new Date(hist.last_used_date).toLocaleString() : `Revision #${idx + 1}`}
                    </span>
                    <p class="font-mono text-gray-200 select-all truncate text-sm">
                      {revealedHistory[idx] ? (hist.password || "(empty)") : "••••••••••••"}
                    </p>
                  </div>
                  <div class="flex items-center gap-1 shrink-0">
                    <button
                      type="button"
                      onclick={() => (revealedHistory[idx] = !revealedHistory[idx])}
                      title={revealedHistory[idx] ? "Hide password" : "Reveal password"}
                      class="text-gray-400 hover:text-brand-400 transition-colors p-1 rounded hover:bg-gray-800 flex items-center justify-center w-7 h-7 cursor-pointer"
                    >
                      {#if revealedHistory[idx]}
                        <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
                          <line x1="1" y1="1" x2="23" y2="23" />
                        </svg>
                      {:else}
                        <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                          <circle cx="12" cy="12" r="3" />
                        </svg>
                      {/if}
                    </button>
                    <CopyButton value={hist.password ?? ""} title="Copy historical password" />
                  </div>
                </div>
              {/each}
            </div>
          {:else}
            <p class="text-xs text-gray-500 text-center py-4">No previous passwords recorded in history.</p>
          {/if}
        {:else}
          <!-- Revision Info -->
          <div class="space-y-2 text-xs py-2">
            <div class="flex items-center justify-between text-gray-400 border-b border-gray-800/60 pb-2">
              <span>Last Modified</span>
              <span class="font-mono text-gray-200">
                {$selectedResource.modified_at ? new Date($selectedResource.modified_at * 1000).toLocaleString() : "Unknown"}
              </span>
            </div>
            <div class="flex items-center justify-between text-gray-400 border-b border-gray-800/60 pb-2">
              <span>Item ID</span>
              <span class="font-mono text-gray-300 text-[11px]">{$selectedResource.id}</span>
            </div>
            <div class="flex items-center justify-between text-gray-400">
              <span>Type</span>
              <span class="font-mono text-gray-200 capitalize">{$selectedResource.resource_type_slug || "Login"}</span>
            </div>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</main>
