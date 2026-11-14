import { writable, get } from "svelte/store";
import type { ProfileSummary, ResourceItem, FolderNode } from "./types";
import { searchResources } from "./tauri";

export const profiles = writable<ProfileSummary[]>([]);
export const activeProfileId = writable<string | null>(null);
export const vaultLocked = writable<boolean>(true);
export const resources = writable<ResourceItem[]>([]);
export const selectedResource = writable<ResourceItem | null>(null);
export const showAddProfile = writable<boolean>(false);
export const showPasswordGen = writable<boolean>(false);
export const showSettings = writable<boolean>(false);
export const syncStatus = writable<"idle" | "syncing" | "offline">("idle");
export const folderTree = writable<FolderNode[]>([]);
// null = "All Items" (no filter) - the only other state is a folder's id.
export const selectedFolderId = writable<string | null>(null);
export const searchQuery = writable<string>("");

/**
 * Re-queries resources matching the currently active search query and folder filter,
 * ensuring background syncs or updates never clobber the user's active folder view.
 */
export async function refreshVaultItems(): Promise<ResourceItem[]> {
  const q = get(searchQuery);
  const folderId = get(selectedFolderId);
  const items = await searchResources(q, folderId);
  resources.set(items);
  return items;
}

// Drives ResourceForm.svelte: null = closed, "create" = blank form (new
// resource, defaulting into whichever folder is currently selected),
// a ResourceItem = edit form pre-filled from that resource,
// { cloneFrom: ResourceItem } = create new resource with values pre-filled from that item.
export type ResourceFormMode = "create" | { cloneFrom: ResourceItem } | ResourceItem | null;
export const resourceFormMode = writable<ResourceFormMode>(null);

export interface ShareModalTarget {
  modelType: "resource" | "folder";
  itemId: string;
  name: string;
}
export const shareModalTarget = writable<ShareModalTarget | null>(null);

const initialTheme = (typeof localStorage !== "undefined" && localStorage.getItem("theme")) || "dark";
export const theme = writable<"dark" | "light">(initialTheme as "dark" | "light");

if (typeof document !== "undefined") {
  theme.subscribe((t) => {
    if (t === "light") {
      document.documentElement.classList.add("light");
      document.documentElement.classList.remove("dark");
    } else {
      document.documentElement.classList.remove("light");
      document.documentElement.classList.add("dark");
    }
    localStorage.setItem("theme", t);
  });
}

export function toggleTheme() {
  theme.update((t) => (t === "dark" ? "light" : "dark"));
}
