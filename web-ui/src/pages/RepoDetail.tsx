import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { api, type Repository, type User } from '../api';
import {
  ArrowLeft, RefreshCw, Settings, Database, GitBranch, Clock,
  Activity, Zap, Save, X, Trash2, Power,
} from 'lucide-react';

function getStoredUser(): User | null {
  try {
    const stored = localStorage.getItem('user');
    return stored ? JSON.parse(stored) : null;
  } catch {
    return null;
  }
}

function formatTimeAgo(isoDate: string): string {
  const diff = Math.max(0, Math.floor((Date.now() - new Date(isoDate).getTime()) / 1000));
  if (diff < 60) return `${diff}s ago`;
  const mins = Math.floor(diff / 60);
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ${mins % 60}m ago`;
  return `${Math.floor(hrs / 24)}d ago`;
}

type EditForm = {
  name: string;
  svn_url: string;
  svn_branch: string;
  svn_username: string;
  git_provider: string;
  git_api_url: string;
  git_repo: string;
  git_branch: string;
  sync_mode: string;
  poll_interval_secs: number;
  lfs_threshold_mb: number;
  auto_merge: boolean;
};

function repoToForm(repo: Repository): EditForm {
  return {
    name: repo.name,
    svn_url: repo.svn_url,
    svn_branch: repo.svn_branch,
    svn_username: repo.svn_username,
    git_provider: repo.git_provider,
    git_api_url: repo.git_api_url,
    git_repo: repo.git_repo,
    git_branch: repo.git_branch,
    sync_mode: repo.sync_mode,
    poll_interval_secs: repo.poll_interval_secs,
    lfs_threshold_mb: repo.lfs_threshold_mb,
    auto_merge: repo.auto_merge,
  };
}

const inputClass =
  'w-full bg-gray-700 border border-gray-600 rounded-md px-3 py-2 text-sm text-gray-100 placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent';

const selectClass =
  'w-full bg-gray-700 border border-gray-600 rounded-md px-3 py-2 text-sm text-gray-100 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent';

export default function RepoDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const user = getStoredUser();
  const isAdmin = user?.role === 'admin';

  const [syncTriggered, setSyncTriggered] = useState(false);
  const [editing, setEditing] = useState(false);
  const [form, setForm] = useState<EditForm | null>(null);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const { data: repo, isLoading, isError, error } = useQuery({
    queryKey: ['repo', id],
    queryFn: () => api.getRepo(id!),
    enabled: !!id,
  });

  const syncMutation = useMutation({
    mutationFn: () => api.triggerRepoSync(id!),
    onSuccess: () => {
      setSyncTriggered(true);
      queryClient.invalidateQueries({ queryKey: ['repo', id] });
      setTimeout(() => setSyncTriggered(false), 3000);
    },
  });

  const updateMutation = useMutation({
    mutationFn: (data: Partial<Repository>) => api.updateRepo(id!, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['repo', id] });
      queryClient.invalidateQueries({ queryKey: ['repos'] });
      setEditing(false);
      setForm(null);
    },
  });

  const toggleMutation = useMutation({
    mutationFn: (enabled: boolean) => api.updateRepo(id!, { enabled }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['repo', id] });
      queryClient.invalidateQueries({ queryKey: ['repos'] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: () => api.deleteRepo(id!),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['repos'] });
      navigate('/repos');
    },
  });

  function startEdit() {
    if (!repo) return;
    setForm(repoToForm(repo));
    setEditing(true);
  }

  function cancelEdit() {
    setEditing(false);
    setForm(null);
  }

  function handleSave() {
    if (!form) return;
    updateMutation.mutate(form);
  }

  function setField<K extends keyof EditForm>(key: K, value: EditForm[K]) {
    setForm((prev) => (prev ? { ...prev, [key]: value } : prev));
  }

  if (isLoading) {
    return <div className="text-center py-8 text-gray-400">Loading repository...</div>;
  }

  if (isError || !repo) {
    return (
      <div className="text-center py-8 text-red-400">
        Error loading repository: {error?.message ?? 'Not found'}
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <Link
            to="/repos"
            className="inline-flex items-center gap-1 text-sm text-gray-400 hover:text-gray-200 transition-colors"
          >
            <ArrowLeft className="w-4 h-4" />
            Back to Repositories
          </Link>
        </div>
      </div>

      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-bold text-gray-100">{repo.name}</h1>
          <span
            className={`inline-flex items-center px-2.5 py-0.5 rounded text-xs font-medium ${
              repo.enabled
                ? 'bg-green-900/50 text-green-300'
                : 'bg-gray-700 text-gray-400'
            }`}
          >
            {repo.enabled ? 'Enabled' : 'Disabled'}
          </span>
        </div>
        <div className="flex items-center gap-3">
          {/* Enable/Disable toggle */}
          <button
            onClick={() => toggleMutation.mutate(!repo.enabled)}
            disabled={toggleMutation.isPending}
            className={`inline-flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
              repo.enabled
                ? 'border border-yellow-600 text-yellow-300 hover:bg-yellow-900/30'
                : 'border border-green-600 text-green-300 hover:bg-green-900/30'
            } disabled:opacity-50`}
          >
            <Power className="w-4 h-4" />
            {repo.enabled ? 'Disable' : 'Enable'}
          </button>

          {!editing ? (
            <button
              onClick={startEdit}
              className="inline-flex items-center gap-2 px-4 py-2 rounded-lg border border-gray-600 hover:border-gray-500 text-gray-300 hover:text-white text-sm font-medium transition-colors"
            >
              <Settings className="w-4 h-4" />
              Edit
            </button>
          ) : (
            <>
              <button
                onClick={handleSave}
                disabled={updateMutation.isPending}
                className="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 disabled:opacity-50 text-white text-sm font-medium transition-colors"
              >
                <Save className="w-4 h-4" />
                {updateMutation.isPending ? 'Saving...' : 'Save'}
              </button>
              <button
                onClick={cancelEdit}
                className="inline-flex items-center gap-2 px-4 py-2 rounded-lg border border-gray-600 hover:border-gray-500 text-gray-300 hover:text-white text-sm font-medium transition-colors"
              >
                <X className="w-4 h-4" />
                Cancel
              </button>
            </>
          )}
          <button
            onClick={() => syncMutation.mutate()}
            disabled={syncMutation.isPending || syncTriggered}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed text-white text-sm font-medium transition-colors"
          >
            <RefreshCw className={`w-4 h-4 ${syncMutation.isPending ? 'animate-spin' : ''}`} />
            {syncTriggered ? 'Sync Triggered' : 'Trigger Sync'}
          </button>
        </div>
      </div>

      {/* Mutation errors */}
      {syncMutation.isError && (
        <div className="bg-red-900/30 border border-red-700 rounded-lg p-4 text-red-300 text-sm">
          Failed to trigger sync: {syncMutation.error?.message}
        </div>
      )}
      {updateMutation.isError && (
        <div className="bg-red-900/30 border border-red-700 rounded-lg p-4 text-red-300 text-sm">
          Failed to save: {updateMutation.error?.message}
        </div>
      )}

      {/* Config Cards - SVN and Git side by side */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {/* SVN Config */}
        <div className="bg-gray-800/60 border border-gray-700 rounded-lg p-5">
          <div className="flex items-center gap-2 mb-4">
            <Database className="w-5 h-5 text-blue-400" />
            <h2 className="text-lg font-semibold text-gray-100">SVN Configuration</h2>
          </div>
          <div className="space-y-3">
            {editing && form ? (
              <>
                <FieldInput label="Name" value={form.name} onChange={(v) => setField('name', v)} />
                <FieldInput label="SVN URL" value={form.svn_url} onChange={(v) => setField('svn_url', v)} />
                <FieldInput label="Branch" value={form.svn_branch} onChange={(v) => setField('svn_branch', v)} />
                <FieldInput label="Username" value={form.svn_username} onChange={(v) => setField('svn_username', v)} />
              </>
            ) : (
              <>
                <ConfigRow label="Name" value={repo.name} />
                <ConfigRow label="URL" value={repo.svn_url} />
                <ConfigRow label="Branch" value={repo.svn_branch} />
                <ConfigRow label="Username" value={repo.svn_username} />
              </>
            )}
          </div>
        </div>

        {/* Git Config */}
        <div className="bg-gray-800/60 border border-gray-700 rounded-lg p-5">
          <div className="flex items-center gap-2 mb-4">
            <GitBranch className="w-5 h-5 text-purple-400" />
            <h2 className="text-lg font-semibold text-gray-100">Git Configuration</h2>
          </div>
          <div className="space-y-3">
            {editing && form ? (
              <>
                <FieldSelect
                  label="Provider"
                  value={form.git_provider}
                  options={[
                    { value: 'github', label: 'GitHub' },
                    { value: 'gitea', label: 'Gitea' },
                  ]}
                  onChange={(v) => setField('git_provider', v)}
                />
                <FieldInput label="API URL" value={form.git_api_url} onChange={(v) => setField('git_api_url', v)} />
                <FieldInput label="Repository" value={form.git_repo} onChange={(v) => setField('git_repo', v)} placeholder="owner/repo" />
                <FieldInput label="Default Branch" value={form.git_branch} onChange={(v) => setField('git_branch', v)} />
              </>
            ) : (
              <>
                <ConfigRow label="Provider" value={repo.git_provider} />
                <ConfigRow label="API URL" value={repo.git_api_url} />
                <ConfigRow label="Repository" value={repo.git_repo} />
                <ConfigRow label="Branch" value={repo.git_branch} />
              </>
            )}
          </div>
        </div>
      </div>

      {/* Sync Settings */}
      <div className="bg-gray-800/60 border border-gray-700 rounded-lg p-5">
        <div className="flex items-center gap-2 mb-4">
          <Activity className="w-5 h-5 text-green-400" />
          <h2 className="text-lg font-semibold text-gray-100">Sync Settings</h2>
        </div>
        {editing && form ? (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <FieldSelect
              label="Sync Mode"
              value={form.sync_mode}
              options={[
                { value: 'direct', label: 'Direct' },
                { value: 'pr', label: 'Pull Request' },
              ]}
              onChange={(v) => setField('sync_mode', v)}
            />
            <FieldNumber label="Poll Interval (s)" value={form.poll_interval_secs} onChange={(v) => setField('poll_interval_secs', v)} min={10} />
            <FieldNumber label="LFS Threshold (MB)" value={form.lfs_threshold_mb} onChange={(v) => setField('lfs_threshold_mb', v)} min={0} />
            <FieldToggle label="Auto Merge" checked={form.auto_merge} onChange={(v) => setField('auto_merge', v)} />
          </div>
        ) : (
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div>
              <span className="text-sm text-gray-400">Sync Mode</span>
              <p className="text-lg text-gray-100 capitalize">{repo.sync_mode}</p>
            </div>
            <div>
              <span className="text-sm text-gray-400">Poll Interval</span>
              <p className="text-lg text-gray-100">{repo.poll_interval_secs}s</p>
            </div>
            <div>
              <span className="text-sm text-gray-400">LFS Threshold</span>
              <p className="text-lg text-gray-100">{repo.lfs_threshold_mb} MB</p>
            </div>
            <div>
              <span className="text-sm text-gray-400">Auto Merge</span>
              <p className="text-lg text-gray-100">{repo.auto_merge ? 'Yes' : 'No'}</p>
            </div>
          </div>
        )}
      </div>

      {/* Quick Stats */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <StatCard
          icon={<Zap className="w-5 h-5 text-yellow-400" />}
          label="LFS Threshold"
          value={`${repo.lfs_threshold_mb} MB`}
        />
        <StatCard
          icon={<Clock className="w-5 h-5 text-blue-400" />}
          label="Created"
          value={new Date(repo.created_at).toLocaleDateString()}
        />
        <StatCard
          icon={<Activity className="w-5 h-5 text-green-400" />}
          label="Last Updated"
          value={formatTimeAgo(repo.updated_at)}
        />
        <StatCard
          icon={<Database className="w-5 h-5 text-purple-400" />}
          label="Created By"
          value={repo.created_by ?? 'System'}
        />
      </div>

      {/* Delete button - admin only */}
      {isAdmin && (
        <div className="border-t border-gray-700 pt-6">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-sm font-medium text-red-400">Danger Zone</h3>
              <p className="text-sm text-gray-500 mt-1">Permanently remove this repository configuration.</p>
            </div>
            <button
              onClick={() => setShowDeleteConfirm(true)}
              className="inline-flex items-center gap-2 px-4 py-2 rounded-lg border border-red-700 text-red-400 hover:bg-red-900/30 text-sm font-medium transition-colors"
            >
              <Trash2 className="w-4 h-4" />
              Delete Repository
            </button>
          </div>
        </div>
      )}

      {/* Delete confirmation modal */}
      {showDeleteConfirm && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
          <div className="bg-gray-800 border border-gray-700 rounded-lg p-6 max-w-md w-full shadow-xl">
            <h3 className="text-lg font-semibold text-gray-100 mb-2">Delete Repository</h3>
            <p className="text-sm text-gray-400 mb-6">
              Are you sure you want to delete <span className="font-semibold text-gray-200">{repo.name}</span>? This cannot be undone.
            </p>
            {deleteMutation.isError && (
              <div className="bg-red-900/30 border border-red-700 rounded-lg p-3 text-red-300 text-sm mb-4">
                Failed to delete: {deleteMutation.error?.message}
              </div>
            )}
            <div className="flex items-center justify-end gap-3">
              <button
                onClick={() => setShowDeleteConfirm(false)}
                className="px-4 py-2 rounded-lg border border-gray-600 text-gray-300 hover:text-white text-sm font-medium transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => deleteMutation.mutate()}
                disabled={deleteMutation.isPending}
                className="px-4 py-2 rounded-lg bg-red-600 hover:bg-red-700 disabled:opacity-50 text-white text-sm font-medium transition-colors"
              >
                {deleteMutation.isPending ? 'Deleting...' : 'Delete'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

/* ---- Sub-components ---- */

function ConfigRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-4">
      <span className="text-sm text-gray-400 flex-shrink-0">{label}</span>
      <span className="text-sm text-gray-200 truncate text-right font-mono">{value}</span>
    </div>
  );
}

function FieldInput({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  return (
    <div>
      <label className="block text-sm text-gray-400 mb-1">{label}</label>
      <input
        type="text"
        className={inputClass}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
      />
    </div>
  );
}

function FieldNumber({
  label,
  value,
  onChange,
  min,
}: {
  label: string;
  value: number;
  onChange: (v: number) => void;
  min?: number;
}) {
  return (
    <div>
      <label className="block text-sm text-gray-400 mb-1">{label}</label>
      <input
        type="number"
        className={inputClass}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        min={min}
      />
    </div>
  );
}

function FieldSelect({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: { value: string; label: string }[];
  onChange: (v: string) => void;
}) {
  return (
    <div>
      <label className="block text-sm text-gray-400 mb-1">{label}</label>
      <select className={selectClass} value={value} onChange={(e) => onChange(e.target.value)}>
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </div>
  );
}

function FieldToggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div>
      <label className="block text-sm text-gray-400 mb-1">{label}</label>
      <button
        type="button"
        onClick={() => onChange(!checked)}
        className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors mt-1 ${
          checked ? 'bg-blue-600' : 'bg-gray-600'
        }`}
      >
        <span
          className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
            checked ? 'translate-x-6' : 'translate-x-1'
          }`}
        />
      </button>
    </div>
  );
}

function StatCard({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  return (
    <div className="bg-gray-800/60 border border-gray-700 rounded-lg p-4">
      <div className="flex items-center gap-2 mb-2">
        {icon}
        <span className="text-sm text-gray-400">{label}</span>
      </div>
      <p className="text-lg font-semibold text-gray-100 capitalize">{value}</p>
    </div>
  );
}
