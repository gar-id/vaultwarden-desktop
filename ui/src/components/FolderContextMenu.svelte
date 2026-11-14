<script lang="ts">
  import type { FolderNode } from "../lib/types";

  interface Props {
    node: FolderNode;
    x: number;
    y: number;
    onClose: () => void;
    onAction: (action: "create-child" | "rename" | "delete" | "share", node: FolderNode) => void;
  }

  let { node, x, y, onClose, onAction }: Props = $props();

  function triggerAction(action: "create-child" | "rename" | "delete" | "share") {
    onAction(action, node);
    onClose();
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
  class="fixed z-50 bg-gray-900 border border-gray-800 rounded-lg shadow-2xl py-1 w-44 text-sm"
  style="left: {x}px; top: {y}px;"
>
  <button
    onclick={() => triggerAction("create-child")}
    class="w-full text-left px-3 py-1.5 text-xs text-gray-300 hover:text-gray-100 hover:bg-gray-800 transition-colors"
  >
    New Subfolder
  </button>
  <button
    onclick={() => triggerAction("rename")}
    class="w-full text-left px-3 py-1.5 text-xs text-gray-300 hover:text-gray-100 hover:bg-gray-800 transition-colors"
  >
    Rename
  </button>
  <button
    onclick={() => triggerAction("share")}
    class="w-full text-left px-3 py-1.5 text-xs text-gray-300 hover:text-gray-100 hover:bg-gray-800 transition-colors"
  >
    Share
  </button>
  <div class="my-1 border-t border-gray-800"></div>
  <button
    onclick={() => triggerAction("delete")}
    class="w-full text-left px-3 py-1.5 text-xs text-brand-400 hover:text-brand-300 hover:bg-gray-800 transition-colors"
  >
    Delete
  </button>
</div>
