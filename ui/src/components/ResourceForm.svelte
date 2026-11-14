<script lang="ts">
  import { resourceFormMode, selectedFolderId, selectedResource, refreshVaultItems } from "../lib/stores";
  import {
    createResource,
    updateResource,
    generatePassword,
    revealSecret,
    updateResourceTags,
  } from "../lib/tauri";
  import { useEscapeToClose } from "../lib/useEscapeToClose";
  import type { ResourceItem } from "../lib/types";

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
    uri: "",
    password: "",
    description: "",
    tags: "",
  });
  let error = $state("");
  let loading = $state(false);
  let loadingExisting = $state(false);
  let revealPassword = $state(false);

  let editingResource = $state<ResourceItem | null>(null);
  let cloneSourceName = $state<string | null>(null);

  $effect(() => {
    const mode = $resourceFormMode;
    error = "";
    revealPassword = false;
    cloneSourceName = null;

    if (mode === null) {
      editingResource = null;
      return;
    }

    if (mode === "create") {
      editingResource = null;
      form = { name: "", username: "", uri: "", password: "", description: "", tags: "" };
      return;
    }

    if (typeof mode === "object" && "cloneFrom" in mode) {
      const src = mode.cloneFrom;
      editingResource = null; // Creating a new resource with copied values
      cloneSourceName = src.name;
      form = {
        name: `${src.name} (Copy)`,
        username: src.username ?? "",
        uri: src.uri ?? "",
        password: "",
        description: src.description ?? "",
        tags: src.tags ? src.tags.join(", ") : "",
      };
      loadingExisting = true;
      revealSecret(src.id)
        .then((secret) => {
          form.password = secret.password ?? "";
          form.description = secret.description ?? form.description;
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
    form = {
      name: mode.name,
      username: mode.username ?? "",
      uri: mode.uri ?? "",
      password: "",
      description: mode.description ?? "",
      tags: mode.tags ? mode.tags.join(", ") : "",
    };
    loadingExisting = true;
    revealSecret(mode.id)
      .then((secret) => {
        form.password = secret.password ?? "";
        form.description = secret.description ?? form.description;
      })
      .catch((e) => {
        error = `Could not load existing secret: ${e instanceof Error ? e.message : String(e)}`;
      })
      .finally(() => {
        loadingExisting = false;
      });
  });

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
      let savedId: string;
      if (editingResource) {
        savedId = editingResource.id;
        await updateResource(editingResource.id, {
          name: form.name,
          username: form.username || null,
          uri: form.uri || null,
          password: form.password,
          description: form.description || null,
        });
      } else {
        let folderId: string | null = null;
        const unsubscribe = selectedFolderId.subscribe((v) => (folderId = v));
        unsubscribe();
        savedId = await createResource({
          name: form.name,
          username: form.username || null,
          uri: form.uri || null,
          password: form.password,
          description: form.description || null,
          folder_parent_id: folderId,
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
          class="bg-gray-900 rounded-lg p-6 w-[480px] space-y-3 shadow-2xl">
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

      <label class="block">
        <span class="text-xs text-gray-400 uppercase tracking-wide">URL / Host</span>
        <input type="text" placeholder="https://example.com or 127.0.0.1:3306" bind:value={form.uri}
          class="mt-1 w-full bg-gray-800 text-gray-100 placeholder-gray-600 rounded px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-brand-500" />
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
