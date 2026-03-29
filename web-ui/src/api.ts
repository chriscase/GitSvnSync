const API_BASE = '/api';

export interface SyncStatus {
  state: string;
  last_sync_at: string | null;
  last_svn_revision: number | null;
  last_git_hash: string | null;
  total_syncs: number;
  total_conflicts: number;
  active_conflicts: number;
  total_errors: number;
  uptime_secs: number;
}

export interface Conflict {
  id: string;
  file_path: string;
  conflict_type: string;
  svn_content: string | null;
  git_content: string | null;
  base_content: string | null;
  svn_revision: number | null;
  git_hash: string | null;
  status: string;
  resolution: string | null;
  resolved_by: string | null;
  created_at: string;
  resolved_at: string | null;
}

export interface AuditEntry {
  id: number;
  action: string;
  direction: string | null;
  svn_rev: number | null;
  git_sha: string | null;
  author: string | null;
  details: string | null;
  created_at: string;
  success: boolean;
}

export interface AuditListResponse {
  entries: AuditEntry[];
  total: number;
}

export interface AuthorMapping {
  svn_username: string;
  name: string;
  email: string;
  github?: string;
}

export interface CommitMapEntry {
  id: number;
  svn_rev: number;
  git_sha: string;
  direction: string;
  synced_at: string;
  svn_author: string;
  git_author: string;
}

export interface CommitMapResponse {
  entries: CommitMapEntry[];
  total: number;
}

export interface SyncRecord {
  id: string;
  svn_rev: number | null;
  git_sha: string | null;
  direction: string;
  author: string;
  message: string;
  timestamp: string;
  synced_at: string;
  status: string;
}

export interface SyncRecordResponse {
  entries: SyncRecord[];
  total: number;
}

export interface ConfigResponse {
  daemon: { poll_interval_secs: number; log_level: string; data_dir: string };
  svn: { url: string; username: string; password: string; trunk_path: string };
  github: { api_url: string; repo: string; token: string; default_branch: string };
  web: { listen: string; auth_mode: string };
  sync: { mode: string; auto_merge: boolean; sync_tags: boolean };
}

async function fetchJson<T>(url: string, options?: RequestInit): Promise<T> {
  const token = localStorage.getItem('session_token');
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
  };

  const res = await fetch(`${API_BASE}${url}`, { ...options, headers });
  if (res.status === 401) {
    localStorage.removeItem('session_token');
    window.location.href = '/login';
    throw new Error('Unauthorized');
  }
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`API error ${res.status}: ${text}`);
  }
  return res.json();
}

export const api = {
  getStatus: () => fetchJson<SyncStatus>('/status'),

  getHealth: () => fetchJson<{ ok: boolean }>('/status/health'),

  getSystemMetrics: async (): Promise<SystemMetrics> => {
    const res = await fetch('/api/status/system');
    if (!res.ok) throw new Error('Failed to fetch system metrics');
    return res.json();
  },

  getConflicts: (status?: string) => {
    const params = new URLSearchParams();
    if (status) params.append('status', status);
    const qs = params.toString();
    return fetchJson<Conflict[]>(`/conflicts${qs ? `?${qs}` : ''}`);
  },

  getConflict: (id: string) => fetchJson<Conflict>(`/conflicts/${id}`),

  resolveConflict: (id: string, resolution: string) =>
    fetchJson<{ ok: boolean }>(`/conflicts/${id}/resolve`, {
      method: 'POST',
      body: JSON.stringify({ resolution }),
    }),

  deferConflict: (id: string) =>
    fetchJson<{ ok: boolean }>(`/conflicts/${id}/defer`, { method: 'POST' }),

  getAuditLog: (limit = 50) =>
    fetchJson<AuditListResponse>(`/audit?limit=${limit}`),

  getIdentityMappings: () => fetchJson<AuthorMapping[]>('/config/identity'),

  updateIdentityMappings: (mappings: AuthorMapping[]) =>
    fetchJson<{ ok: boolean }>('/config/identity', {
      method: 'PUT',
      body: JSON.stringify({ mappings }),
    }),

  getConfig: () => fetchJson<ConfigResponse>('/config'),

  getCommitMap: (limit = 100) =>
    fetchJson<CommitMapResponse>(`/commit-map?limit=${limit}`),

  getSyncRecords: (limit = 100) =>
    fetchJson<SyncRecordResponse>(`/sync-records?limit=${limit}`),

  seedData: () =>
    fetchJson<{ ok: boolean; message: string }>('/seed', { method: 'POST' }),

  testSvnConnection: (data: { url: string; username: string }) =>
    fetchJson<{ ok: boolean; message: string }>('/setup/test-svn', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  testGitConnection: (data: { api_url: string; repo: string; provider: string }) =>
    fetchJson<{ ok: boolean; message: string }>('/setup/test-git', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  applyConfig: (data: Record<string, unknown>) =>
    fetchJson<{ ok: boolean; message: string; warnings: string[] }>('/setup/apply', {
      method: 'POST',
      body: JSON.stringify(data),
    }),

  startImport: () =>
    fetchJson<{ ok: boolean; message: string }>('/setup/import', {
      method: 'POST',
    }),

  getImportStatus: () =>
    fetchJson<ImportStatus>('/setup/import/status'),

  cancelImport: () =>
    fetchJson<{ ok: boolean; message: string }>('/setup/import/cancel', {
      method: 'POST',
    }),

  login: (password: string) =>
    fetchJson<{ token: string }>('/auth/login', {
      method: 'POST',
      body: JSON.stringify({ password }),
    }),

  logout: () => {
    const token = localStorage.getItem('session_token');
    return fetchJson<{ ok: boolean }>('/auth/logout', {
      method: 'POST',
      body: JSON.stringify({ token: token ?? '' }),
    });
  },
};

export interface SystemMetrics {
  disk_free_bytes: number;
  disk_total_bytes: number;
  disk_usage_percent: number;
  mem_used_bytes: number;
  mem_total_bytes: number;
  mem_usage_percent: number;
  cpu_load_1m: number;
  cpu_load_5m: number;
  cpu_load_15m: number;
  git_push_active: boolean;
  git_push_pid: number | null;
  git_push_elapsed_secs: number | null;
  data_dir_size_bytes: number;
}

export interface VerificationResult {
  ok: boolean;
  mismatched_files: string[];
  missing_files: string[];
  extra_files: string[];
  message: string;
}

export interface ImportStatus {
  phase: 'idle' | 'connecting' | 'importing' | 'verifying' | 'final_push' | 'completed' | 'failed' | 'cancelled';
  current_rev: number;
  total_revs: number;
  commits_created: number;
  current_file_count: number;
  lfs_unique_count: number;
  files_skipped: number;
  batches_pushed: number;
  push_started_at: string | null;
  verification: VerificationResult | null;
  errors: string[];
  log_lines: string[];
  started_at: string | null;
  completed_at: string | null;
}
