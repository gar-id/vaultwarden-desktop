<script lang="ts">
  import { selectedFolderId } from "../lib/stores";
  import type { FolderNode } from "../lib/types";
  import FolderTreeNode from "./FolderTreeNode.svelte";

  interface Props {
    node: FolderNode;
    depth: number;
    onContextMenu: (e: MouseEvent, node: FolderNode) => void;
    onDropItem?: (modelType: "folder" | "resource", itemId: string, targetFolderId: string | null) => Promise<void>;
  }

  let { node, depth, onContextMenu, onDropItem }: Props = $props();

  // Start expanded - most vaults have few enough folders that collapsing
  // by default would hide more than it helps; collapsing is still useful
  // once a subtree gets deep, so it's per-node state, not global.
  let expanded = $state(true);
  let isDragOver = $state(false);

  function select() {
    selectedFolderId.set(node.id);
  }

  function handleDragStart(e: DragEvent) {
    if (!e.dataTransfer) return;
    e.dataTransfer.setData("application/json", JSON.stringify({ type: "folder", id: node.id }));
    e.dataTransfer.effectAllowed = "move";
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    if (!e.dataTransfer) return;
    e.dataTransfer.dropEffect = "move";
    isDragOver = true;
  }

  function handleDragLeave() {
    isDragOver = false;
  }

  async function handleDrop(e: DragEvent) {
    e.preventDefault();
    isDragOver = false;
    if (!e.dataTransfer || !onDropItem) return;
    const raw = e.dataTransfer.getData("application/json");
    if (!raw) return;
    try {
      const data = JSON.parse(raw);
      if (data.type === "folder" && data.id === node.id) {
        return; // Can't drop folder into itself
      }
      await onDropItem(data.type, data.id, node.id);
    } catch (err) {
      console.error("Drop error:", err);
    }
  }
</script>

<li>
  <div
    draggable="true"
    ondragstart={handleDragStart}
    ondragover={handleDragOver}
    ondragleave={handleDragLeave}
    ondrop={handleDrop}
    class="flex items-center gap-1 px-1 transition-colors rounded
           {isDragOver ? 'bg-brand-900/60 ring-1 ring-brand-500' : $selectedFolderId === node.id ? 'bg-gray-800' : 'hover:bg-gray-800/50'}"
    style="padding-left: {depth * 14 + 4}px"
    oncontextmenu={(e) => { e.preventDefault(); onContextMenu(e, node); }}
    role="presentation"
  >
    {#if node.children.length > 0}
      <button
        onclick={() => (expanded = !expanded)}
        class="text-gray-500 hover:text-gray-300 w-4 h-4 flex items-center justify-center shrink-0"
        title={expanded ? "Collapse" : "Expand"}
      >
        <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3 transition-transform {expanded ? 'rotate-90' : ''}"
             viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
          <path d="M9 18l6-6-6-6" />
        </svg>
      </button>
    {:else}
      <span class="w-4 h-4 shrink-0"></span>
    {/if}

    <button
      onclick={select}
      class="flex-1 text-left py-1.5 text-sm truncate transition-colors
             {$selectedFolderId === node.id ? 'text-gray-100' : 'text-gray-400 hover:text-gray-200'}"
      title={node.name}
    >
      {node.name}
    </button>

    {#if node.resource_count > 0}
      <span class="text-xs text-gray-600 pr-2 shrink-0">{node.resource_count}</span>
    {/if}
  </div>

  {#if expanded && node.children.length > 0}
    <ul>
      {#each node.children as child}
        <FolderTreeNode node={child} depth={depth + 1} {onContextMenu} {onDropItem} />
      {/each}
    </ul>
  {/if}
</li>
