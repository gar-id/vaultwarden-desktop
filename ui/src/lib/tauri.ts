import { invoke } from "@tauri-apps/api/core";
import type {
  ProfileSummary,
  ResourceItem,
  PasswordRules,
  RevealedSecret,
  TotpCode,
  FolderNode,
  NewResourceInput,
  ResourceUpdateInput,
} from "./types";

export const listProfiles = () => invoke<ProfileSummary[]>("list_profiles");

// Fingerprint and server public key are never user input - the backend
// derives the fingerprint from the private key and fetches the server's
// public key from its unauthenticated /auth/verify.json endpoint, exactly
// like the real Passbolt browser extension does on first connecting.
export const addProfile = (input: {
  name: string;
  server_url: string;
  user_email: string;
  master_password?: string;
  armored_user_private_key?: string;
}) => invoke<ProfileSummary>("add_profile", { input });

export const discoverServer = (serverUrl: string) =>
  invoke<{ fingerprint: string; public_key: string }>("discover_server", { serverUrl });
export const removeProfile = (profileId: string) =>
  invoke<void>("remove_profile", { profileId });
export const switchProfile = (profileId: string) =>
  invoke<string>("switch_profile", { profileId });

export const unlockVault = (passphrase: string) =>
  invoke<{ success: boolean; message: string; online: boolean }>("unlock_vault", { passphrase });
export const lockVault = () => invoke<void>("lock_vault");

export const syncVault = () =>
  invoke<import("./types").SyncResultDto>("sync_vault");

export const searchResources = (query: string, folderId?: string | null) =>
  invoke<ResourceItem[]>("search_resources", { query, folderId: folderId ?? null });
export const listFolderTree = () => invoke<FolderNode[]>("list_folder_tree");
export const revealSecret = (resourceId: string) =>
  invoke<RevealedSecret>("reveal_secret", { resourceId });
export const getTotpCode = (resourceId: string) =>
  invoke<TotpCode>("get_totp_code", { resourceId });
export const copySecret = (resourceId: string) =>
  invoke<void>("copy_secret", { resourceId });
export const generatePassword = (rules: PasswordRules) =>
  invoke<string>("generate_password", { rules });

// Write commands - all require VaultState::OnlineReady on the backend;
// a locked or offline vault rejects these immediately rather than
// silently queuing a write that could never be reconciled later.
export const createResource = (input: NewResourceInput) =>
  invoke<string>("create_resource", { input });
export const updateResource = (resourceId: string, input: ResourceUpdateInput) =>
  invoke<void>("update_resource", { resourceId, input });
export const deleteResource = (resourceId: string) =>
  invoke<void>("delete_resource", { resourceId });

export const createFolder = (name: string, parentId?: string | null) =>
  invoke<string>("create_folder", { name, parentId: parentId ?? null });

export const renameFolder = (folderId: string, name: string) =>
  invoke<void>("rename_folder", { folderId, name });

export const deleteFolder = (folderId: string) =>
  invoke<void>("delete_folder", { folderId });

export const moveItem = (
  modelType: "folder" | "resource",
  itemId: string,
  targetParentId?: string | null
) => invoke<void>("move_item", { modelType, itemId, targetParentId: targetParentId ?? null });

export const toggleFavorite = (resourceId: string) =>
  invoke<boolean>("toggle_favorite", { resourceId });

export const recordUserActivity = () => invoke<void>("record_user_activity");

export const searchShareTargets = (query: string) =>
  invoke<import("./types").SearchAroResult[]>("search_share_targets", { query });

export const getItemPermissions = (modelType: "resource" | "folder", itemId: string) =>
  invoke<import("./types").ItemPermissionDto[]>("get_item_permissions", { modelType, itemId });

export const shareItem = (
  modelType: "resource" | "folder",
  itemId: string,
  changes: import("./types").ShareChangeInput[]
) => invoke<void>("share_item", { modelType, itemId, changes });

export const listVaultTags = () => invoke<string[]>("list_vault_tags");

export const updateResourceTags = (resourceId: string, tags: string[]) =>
  invoke<void>("update_resource_tags", { resourceId, tags });

export const getResourceComments = (resourceId: string) =>
  invoke<import("./types").ResourceComment[]>("get_resource_comments", { resourceId });

export const addResourceComment = (resourceId: string, content: string) =>
  invoke<import("./types").ResourceComment>("add_resource_comment", { resourceId, content });

export const deleteResourceComment = (commentId: string) =>
  invoke<void>("delete_resource_comment", { commentId });

export const getAppSettings = () =>
  invoke<import("./types").AppSettings>("get_app_settings");

export const setAppSettings = (settings: import("./types").AppSettings) =>
  invoke<void>("set_app_settings", { settings });

export const forceResync = () =>
  invoke<import("./types").SyncResultDto>("force_resync");

export const clearLocalCache = () =>
  invoke<import("./types").SyncResultDto>("clear_local_cache");

export const getResourceActivity = (resourceId: string) =>
  invoke<import("./types").ResourceActivity[]>("get_resource_activity", { resourceId });
