<script lang="ts">
  import type { ResourceItem } from "../lib/types";
  import { copySecret } from "../lib/tauri";

  interface Props {
    item: ResourceItem;
    x: number;
    y: number;
    onClose: () => void;
    onAction: (action: "edit" | "clone" | "delete" | "share", item: ResourceItem) => void;
  }

  let { item, x, y, onClose, onAction }: Props = $props();

  let copiedField = $state<string | null>(null);

  // Prevent menu from overflowing off-screen edges
  let posX = $derived(typeof window !== "undefined" ? Math.min(x, window.innerWidth - 220) : x);
  let posY = $derived(typeof window !== "undefined" ? Math.min(y, window.innerHeight - 280) : y);

  function triggerAction(action: "edit" | "clone" | "delete" | "share") {
    onAction(action, item);
    onClose();
  }

  async function handleCopyPassword() {
    try {
      await copySecret(item.id);
      copiedField = "password";
      setTimeout(() => {
        copiedField = null;
        onClose();
      }, 700);
    } catch (e) {
      console.error(e);
      onClose();
    }
  }

  async function handleCopyUsername() {
    if (!item.username) return;
    try {
      await navigator.clipboard.writeText(item.username);
      copiedField = "username";
      setTimeout(() => {
        copiedField = null;
        onClose();
      }, 700);
    } catch (e) {
      console.error(e);
      onClose();
    }
  }

  async function handleCopyUri() {
    if (!item.uri) return;
    try {
      await navigator.clipboard.writeText(item.uri);
      copiedField = "uri";
      setTimeout(() => {
        copiedField = null;
        onClose();
      }, 700);
    } catch (e) {
      console.error(e);
      onClose();
    }
  }
</script>

<!-- Backdrop to dismiss on outside click -->
<div
  class="fixed inset-0 z-40"
  onclick={onClose}
  oncontextmenu={(e) => { e.preventDefault(); onClose(); }}
  role="presentation"
></div>

<div
  class="fixed z-50 bg-gray-900 border border-gray-800 rounded-lg shadow-2xl py-1 w-52 text-sm select-none"
  style="left: {posX}px; top: {posY}px;"
>
  <button
    onclick={handleCopyPassword}
    class="w-full text-left px-3 py-1.5 text-xs text-gray-300 hover:text-gray-100 hover:bg-gray-800 transition-colors flex items-center justify-between"
  >
    <div class="flex items-center gap-2">
      <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <rect x="9" y="9" width="12" height="12" rx="2" />
        <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
      </svg>
      <span>Copy Password</span>
    </div>
    {#if copiedField === "password"}
      <span class="text-[10px] text-brand-400 font-medium">Copied!</span>
    {/if}
  </button>

  {#if item.username}
    <button
      onclick={handleCopyUsername}
      class="w-full text-left px-3 py-1.5 text-xs text-gray-300 hover:text-gray-100 hover:bg-gray-800 transition-colors flex items-center justify-between"
    >
      <div class="flex items-center gap-2">
        <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
          <circle cx="12" cy="7" r="4" />
        </svg>
        <span>Copy Username</span>
      </div>
      {#if copiedField === "username"}
        <span class="text-[10px] text-brand-400 font-medium">Copied!</span>
      {/if}
    </button>
  {/if}

  {#if item.uri}
    <button
      onclick={handleCopyUri}
      class="w-full text-left px-3 py-1.5 text-xs text-gray-300 hover:text-gray-100 hover:bg-gray-800 transition-colors flex items-center justify-between"
    >
      <div class="flex items-center gap-2">
        <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10" />
          <line x1="2" y1="12" x2="22" y2="12" />
          <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
        </svg>
        <span>Copy URL</span>
      </div>
      {#if copiedField === "uri"}
        <span class="text-[10px] text-brand-400 font-medium">Copied!</span>
      {/if}
    </button>
  {/if}

  <div class="my-1 border-t border-gray-800"></div>

  <button
    onclick={() => triggerAction("edit")}
    class="w-full text-left px-3 py-1.5 text-xs text-gray-300 hover:text-gray-100 hover:bg-gray-800 transition-colors flex items-center gap-2"
  >
    <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
    </svg>
    <span>Edit</span>
  </button>

  <button
    onclick={() => triggerAction("clone")}
    class="w-full text-left px-3 py-1.5 text-xs text-gray-300 hover:text-gray-100 hover:bg-gray-800 transition-colors flex items-center gap-2"
  >
    <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
      <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
    </svg>
    <span>Clone</span>
  </button>

  <button
    onclick={() => triggerAction("share")}
    class="w-full text-left px-3 py-1.5 text-xs text-gray-300 hover:text-gray-100 hover:bg-gray-800 transition-colors flex items-center gap-2"
  >
    <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5 text-gray-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <circle cx="18" cy="5" r="3" />
      <circle cx="6" cy="12" r="3" />
      <circle cx="18" cy="19" r="3" />
      <line x1="8.59" y1="13.51" x2="15.42" y2="17.49" />
      <line x1="15.41" y1="6.51" x2="8.59" y2="10.49" />
    </svg>
    <span>Share</span>
  </button>

  <div class="my-1 border-t border-gray-800"></div>

  <button
    onclick={() => triggerAction("delete")}
    class="w-full text-left px-3 py-1.5 text-xs text-brand-400 hover:text-brand-300 hover:bg-gray-800 transition-colors flex items-center gap-2"
  >
    <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5 text-brand-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <polyline points="3 6 5 6 21 6" />
      <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
    </svg>
    <span>Delete</span>
  </button>
</div>
