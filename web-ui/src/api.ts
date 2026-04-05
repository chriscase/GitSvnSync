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
