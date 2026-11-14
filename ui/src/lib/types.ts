export interface ProfileSummary {
  id: string;
  name: string;
  server_url: string;
  user_email: string;
  last_sync_timestamp: number;
}

export interface ResourceItem {
  id: string;
  name: string;
  username: string | null;
  uri: string | null;
  description: string | null;
  modified_at: number;
  folder_parent_id: string | null;
  resource_type_slug: string | null;
  is_favorite: boolean;
  tags: string[];
}

export interface CommentAuthor {
  id: string;
  username: string;
  profile?: {
    first_name?: string | null;
    last_name?: string | null;
  } | null;
}

export interface ResourceComment {
  id: string;
  parent_id: string | null;
  foreign_id: string | null;
  content: string;
  created: string;
  modified: string | null;
  created_by: string | null;
  creator?: CommentAuthor | null;
}

export interface FolderNode {
  id: string;
  name: string;
  children: FolderNode[];
  resource_count: number;
}

export interface RevealedSecret {
  password: string | null;
  description: string | null;
  has_totp: boolean;
}

export interface TotpCode {
  code: string;
  expires_in_secs: number;
}

export interface NewResourceInput {
  name: string;
  username: string | null;
  uri: string | null;
  password: string;
  description: string | null;
  folder_parent_id: string | null;
}

export interface ResourceUpdateInput {
  name: string;
  username: string | null;
  uri: string | null;
  password: string;
  description: string | null;
}

export interface PasswordRules {
  length: number;
  uppercase: boolean;
  lowercase: boolean;
  numbers: boolean;
  symbols: boolean;
}

export interface SearchAroResult {
  id: string;
  label: string;
  sublabel: string;
  kind: "User" | "Group";
  has_key: boolean;
}

export interface ItemPermissionDto {
  id: string | null;
  aro: "User" | "Group";
  aro_foreign_key: string;
  name: string;
  identifier: string;
  permission_type: number;
}

export interface ShareChangeInput {
  id?: string | null;
  aro: string;
  aro_foreign_key: string;
  permission_type: number;
  is_new: boolean;
}

export interface AppSettings {
  auto_lock_secs: number;
  clipboard_clear_secs: number;
}

export interface SyncResultDto {
  state: string;
  added: number;
  updated: number;
  error: string | null;
}

export interface ActivityUserProfile {
  first_name?: string | null;
  last_name?: string | null;
}

export interface ActivityUser {
  id?: string | null;
  username?: string | null;
  profile?: ActivityUserProfile | null;
}

export interface ResourceActivity {
  id?: string | null;
  action_log_id?: string | null;
  action_id?: string | null;
  context?: string | null;
  type?: string | null;
  status?: number | null;
  created?: string | null;
  user_id?: string | null;
  user?: ActivityUser | null;
}
