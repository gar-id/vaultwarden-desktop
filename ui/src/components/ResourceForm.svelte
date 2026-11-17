<script lang="ts">
  import {
    resourceFormMode,
    selectedFolderId,
    selectedResource,
    refreshVaultItems,
    folderTree,
  } from "../lib/stores";
  import {
    createResource,
    updateResource,
    generatePassword,
    revealSecret,
    updateResourceTags,
  } from "../lib/tauri";
  import { useEscapeToClose } from "../lib/useEscapeToClose";
  import type { ResourceItem, FolderNode } from "../lib/types";

  // useEscapeToClose expects a boolean Writable; resourceFormMode carries
  // the richer "create" | ResourceItem | null shape VaultList/DetailPane
  // need to know which mode opened it, so this derives a boolean view
  // over the same store rather than duplicating open/close state.
  const isOpenAsBoolean = {
    subscribe: (run: (v: boolean) => void) =>
      resourceFormMode.subscribe((v) => run(v !== null)),
    set: (v: boolean) => {
      if (!v) resourceFormMode.set(null);
    },
  };
  useEscapeToClose(isOpenAsBoolean as import("svelte/store").Writable<boolean>);

  let form = $state({
    name: "",
    username: "",
    uris: [""],
    password: "",
    totp: "",
    description: "",
    folderId: "",
    tags: "",
  });
  let customFields = $state<{ name: string; value: string; type: number }[]>([]);
  let error = $state("");
  let loading = $state(false);
  let loadingExisting = $state(false);
  let revealPassword = $state(false);

  let editingResource = $state<ResourceItem | null>(null);
  let cloneSourceName = $state<string | null>(null);

  function flattenFolders(nodes: FolderNode[], prefix = ""): { id: string; name: string }[] {
    let result: { id: string; name: string }[] = [];
    for (const node of nodes) {
      const label = prefix ? `${prefix} / ${node.name}` : node.name;
      result.push({ id: node.id, name: label });
      if (node.children && node.children.length > 0) {
        result = result.concat(flattenFolders(node.children, label));
      }
    }
    return result;
  }

  let folderOptions = $derived(flattenFolders($folderTree));

  $effect(() => {
    const mode = $resourceFormMode;
    error = "";
    revealPassword = false;
    cloneSourceName = null;

    if (mode === null) {
      editingResource = null;
      customFields = [];
      return;
    }

    if (mode === "create") {
      editingResource = null;
      customFields = [];
      const curFolder = $selectedFolderId;
      const initialFolder = (curFolder && curFolder !== "__favorites__") ? curFolder : "";
      form = {
        name: "",
        username: "",
        uris: [""],
        password: "",
        totp: "",
        description: "",
        folderId: initialFolder,
        tags: "",
      };
      return;
    }

    if (typeof mode === "object" && "cloneFrom" in mode) {
      const src = mode.cloneFrom;
      editingResource = null; // Creating a new resource with copied values
      cloneSourceName = src.name;
      customFields = [];
      form = {
        name: `${src.name} (Copy)`,
        username: src.username ?? "",
        uris: [src.uri ?? ""],
        password: "",
        totp: "",
        description: src.description ?? "",
        folderId: src.folder_parent_id ?? "",
        tags: src.tags ? src.tags.join(", ") : "",
      };
      loadingExisting = true;
      revealSecret(src.id)
        .then((secret) => {
          form.password = secret.password ?? "";
          form.totp = secret.totp_secret ?? "";
          if (secret.uris && secret.uris.length > 0) form.uris = [...secret.uris];
          if (secret.description !== undefined && secret.description !== null) {
            form.description = secret.description;
          }
          if (secret.fields && secret.fields.length > 0) {
            customFields = secret.fields.map((f) => ({
              name: f.name || "",
              value: f.value || "",
              type: f.type ?? 0,
            }));
          }
        })
        .catch((e) => {
          error = `Could not load secret for clone: ${e instanceof Error ? e.message : String(e)}`;
        })
        .finally(() => {
          loadingExisting = false;
        });
      return;
    }

    // Edit mode: pre-fill from the resource, but the password/description
    // fields need the actual decrypted secret, not just the cached
    // plaintext row - same lazy-decrypt-on-open pattern DetailPane uses.
    editingResource = mode;
    customFields = [];
    form = {
      name: mode.name,
      username: mode.username ?? "",
      uris: [mode.uri ?? ""],
      password: "",
      totp: "",
      description: mode.description ?? "",
      folderId: mode.folder_parent_id ?? "",
      tags: mode.tags ? mode.tags.join(", ") : "",
    };
    loadingExisting = true;
    revealSecret(mode.id)
      .then((secret) => {
        form.password = secret.password ?? "";
        form.totp = secret.totp_secret ?? "";
        if (secret.uris && secret.uris.length > 0) form.uris = [...secret.uris];
        if (secret.description !== undefined && secret.description !== null) {
          form.description = secret.description;
        }
        if (secret.fields && secret.fields.length > 0) {
          customFields = secret.fields.map((f) => ({
            name: f.name || "",
            value: f.value || "",
            type: f.type ?? 0,
          }));
        }
      })
      .catch((e) => {
        error = `Could not load existing secret: ${e instanceof Error ? e.message : String(e)}`;
      })
      .finally(() => {
        loadingExisting = false;
      });
  });

  function addCustomField() {
    customFields = [...customFields, { name: "", value: "", type: 0 }];
  }

  function addUri() {
    form.uris = [...form.uris, ""];
  }

  function removeUri(idx: number) {
    form.uris = form.uris.filter((_, i) => i !== idx);
  }

  function removeCustomField(idx: number) {
    customFields = customFields.filter((_, i) => i !== idx);
  }

  async function handleGeneratePassword() {
    try {
      form.password = await generatePassword({
        length: 20,
        uppercase: true,
        lowercase: true,
        numbers: true,
        symbols: false,
      });
      revealPassword = true;
    } catch (e) {
      console.error("ResourceForm: generate_password failed:", e);
    }
  }

  async function refreshList() {
    try {
      await refreshVaultItems();
    } catch (e) {
      console.error("ResourceForm: failed to refresh list after write:", e);
    }
  }

  async function handleSubmit() {
    if (loadingExisting) return;
    if (!form.name.trim()) {
      error = "Name is required";
      return;
    }
    if (!form.password) {
      error = "Password is required";
      return;
    }

    loading = true;
    error = "";
    try {
      const formattedFields = customFields
        .filter((f) => f.name.trim() !== "" || f.value.trim() !== "")
        .map((f) => ({
          name: f.name.trim(),
          value: f.value,
          type: Number(f.type),
        }));

      let savedId: string;
      if (editingResource) {
        savedId = editingResource.id;
        await updateResource(editingResource.id, {
          name: form.name,
          username: form.username || null,
          uris: form.uris.map((u) => u.trim()).filter(Boolean),
          password: form.password,
          description: form.description || null,
          folder_parent_id: form.folderId || null,
          totp_secret: form.totp.trim() || null,
          fields: formattedFields,
        });
      } else {
        savedId = await createResource({
          name: form.name,
          username: form.username || null,
          uris: form.uris.map((u) => u.trim()).filter(Boolean),
          password: form.password,
          description: form.description || null,
          folder_parent_id: form.folderId || null,
          totp_secret: form.totp.trim() || null,
          fields: formattedFields,
        });
      }

      const tagsList = form.tags
        .split(",")
        .map((t) => t.trim().replace(/^#/, ""))
        .filter(Boolean);
      try {
        await updateResourceTags(savedId, tagsList);
      } catch (tagErr) {
        console.warn("Could not save tags:", tagErr);
      }

      await refreshList();
      resourceFormMode.set(null);
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function handleClose() {
    resourceFormMode.set(null);
  }
</script>

{#if $resourceFormMode !== null}
  <div class="fixed inset-x-0 bottom-0 top-9 bg-black/70 flex items-center justify-center z-50">
    <form onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}
          class="bg-gray-900 rounded-lg p-6 w-[520px] max-h-[90vh] overflow-y-auto space-y-3 shadow-2xl">
      <h2 class="text-lg font-semibold text-gray-100">
        {editingResource ? `Edit "${editingResource.name}"` : cloneSourceName ? `Clone "${cloneSourceName}"` : "New Credential"}
      </h2>

      {#if loadingExisting}
        <p class="text-xs text-gray-500">Loading existing secret...</p>
      {/if}

      <label class="block">
        <span class="text-xs text-gray-400 uppercase tracking-wide">Name</span>
        <input type="text" placeholder="GitHub Production" bind:value={form.name}
          class="mt-1 w-full bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500" />
      </label>

      <label class="block">
        <span class="text-xs text-gray-400 uppercase tracking-wide">Username</span>
        <input type="text" placeholder="bob@example.com" bind:value={form.username}
          class="mt-1 w-full bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500" />
      </label>

      <div class="space-y-2">
        <div class="flex items-center justify-between">
          <span class="text-xs text-gray-400 uppercase tracking-wide">URL / Host</span>
          <button type="button" onclick={addUri}
            class="text-xs text-brand-400 hover:text-brand-300 font-medium cursor-pointer">
            + Add URL
          </button>
        </div>
        {#each form.uris as _, i}
          <div class="flex gap-2">
            <input type="text" placeholder="https://example.com or 127.0.0.1:3306" bind:value={form.uris[i]}
              class="flex-1 bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500" />
            {#if form.uris.length > 1}
              <button type="button" onclick={() => removeUri(i)} title="Remove URL"
                class="px-2 text-gray-500 hover:text-red-400">
                <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            {/if}
          </div>
        {/each}
      </div>

      <label class="block">
        <span class="text-xs text-gray-400 uppercase tracking-wide">Folder</span>
        <select
          bind:value={form.folderId}
          class="mt-1 w-full bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500 cursor-pointer"
        >
          <option value="">(No Folder)</option>
          {#each folderOptions as f}
            <option value={f.id}>{f.name}</option>
          {/each}
        </select>
      </label>

      <label class="block">
        <span class="text-xs text-gray-400 uppercase tracking-wide">Password</span>
        <div class="mt-1 flex gap-2">
          <input type={revealPassword ? "text" : "password"} placeholder="Password" bind:value={form.password}
            class="flex-1 bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-3 py-1.5 text-sm font-mono outline-none focus:ring-1 focus:ring-brand-500" />
          <button type="button" onclick={() => (revealPassword = !revealPassword)}
            class="px-2 text-xs text-gray-400 hover:text-gray-200 transition-colors">
            {revealPassword ? "Hide" : "Show"}
          </button>
          <button type="button" onclick={handleGeneratePassword}
            class="px-2 text-xs text-brand-400 hover:text-brand-300 transition-colors whitespace-nowrap">
            Generate
          </button>
        </div>
      </label>

      <label class="block">
        <span class="text-xs text-gray-400 uppercase tracking-wide">Authenticator Key (TOTP)</span>
        <input type="text" placeholder="Base32 secret or otpauth:// URL" bind:value={form.totp}
          class="mt-1 w-full bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-3 py-1.5 text-sm font-mono outline-none focus:ring-1 focus:ring-brand-500" />
        <span class="text-[11px] text-gray-500">Leave empty to remove the current authenticator.</span>
      </label>

      <!-- Custom Fields Section -->
      <div class="space-y-2 pt-1 border-t border-gray-800">
        <div class="flex items-center justify-between">
          <span class="text-xs text-gray-400 uppercase tracking-wide">Custom Fields</span>
          <button
            type="button"
            onclick={addCustomField}
            class="text-xs text-brand-400 hover:text-brand-300 font-medium flex items-center gap-1 cursor-pointer"
          >
            <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
            Add Field
          </button>
        </div>

        {#if customFields.length > 0}
          <div class="space-y-2 max-h-48 overflow-y-auto pr-1">
            {#each customFields as field, i}
              <div class="bg-gray-800/70 p-2.5 rounded border border-gray-700/60 space-y-2">
                <div class="flex items-center gap-2">
                  <select
                    bind:value={field.type}
                    class="bg-gray-800 text-gray-200 border border-gray-700 text-xs rounded px-2 py-1 outline-none focus:ring-1 focus:ring-brand-500"
                  >
                    <option value={0}>Text</option>
                    <option value={1}>Hidden</option>
                    <option value={2}>Boolean</option>
                  </select>
                  <input
                    type="text"
                    placeholder="Field name"
                    bind:value={field.name}
                    class="flex-1 bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-2.5 py-1 text-xs outline-none focus:ring-1 focus:ring-brand-500"
                  />
                  <button
                    type="button"
                    onclick={() => removeCustomField(i)}
                    title="Remove field"
                    class="text-gray-500 hover:text-red-400 p-1"
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <line x1="18" y1="6" x2="6" y2="18"></line>
                      <line x1="6" y1="6" x2="18" y2="18"></line>
                    </svg>
                  </button>
                </div>

                {#if field.type === 2}
                  <label class="flex items-center gap-2 text-xs text-gray-300 cursor-pointer pl-1">
                    <input
                      type="checkbox"
                      checked={field.value === "true" || field.value === "1"}
                      onchange={(e) => field.value = e.currentTarget.checked ? "true" : "false"}
                      class="rounded bg-gray-700 border-gray-600 text-brand-600 focus:ring-brand-500"
                    />
                    <span>{field.value === "true" || field.value === "1" ? "True / Yes" : "False / No"}</span>
                  </label>
                {:else if field.type === 1}
                  <input
                    type="password"
                    placeholder="Secret value"
                    bind:value={field.value}
                    class="w-full bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-2.5 py-1 text-xs outline-none focus:ring-1 focus:ring-brand-500 font-mono"
                  />
                {:else}
                  <input
                    type="text"
                    placeholder="Field value"
                    bind:value={field.value}
                    class="w-full bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-2.5 py-1 text-xs outline-none focus:ring-1 focus:ring-brand-500"
                  />
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <label class="block">
        <span class="text-xs text-gray-400 uppercase tracking-wide">Tags</span>
        <input type="text" placeholder="production, database, aws (comma-separated)" bind:value={form.tags}
          class="mt-1 w-full bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500" />
      </label>

      <label class="block">
        <span class="text-xs text-gray-400 uppercase tracking-wide">Notes</span>
        <textarea bind:value={form.description} rows={3}
          placeholder="Optional notes"
          class="mt-1 w-full bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500 resize-none"></textarea>
      </label>

      {#if editingResource}
        <p class="text-xs text-gray-500">
          Saving re-encrypts this secret for everyone who currently has access to it.
        </p>
      {/if}

      {#if error}
        <p class="text-red-400 text-xs">{error}</p>
      {/if}

      <div class="flex justify-end gap-2 pt-2">
        <button type="button" onclick={handleClose}
          class="px-4 py-1.5 text-sm text-gray-400 hover:text-gray-200 transition-colors">Cancel</button>
        <button type="submit" disabled={loading || loadingExisting}
          class="px-4 py-1.5 text-sm bg-brand-600 hover:bg-brand-500 disabled:opacity-50 rounded font-medium transition-colors">
          {loading ? "Saving..." : editingResource ? "Save Changes" : cloneSourceName ? "Create Clone" : "Create"}
        </button>
      </div>
    </form>
  </div>
{/if}
