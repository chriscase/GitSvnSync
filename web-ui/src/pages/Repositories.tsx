import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { api, type Repository, type User } from '../api';
import { GitBranch, Plus, Database, Clock, X } from 'lucide-react';

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

const inputClass =
  'w-full bg-gray-700 border border-gray-600 rounded-md px-3 py-2 text-sm text-gray-100 placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent';

const selectClass =
  'w-full bg-gray-700 border border-gray-600 rounded-md px-3 py-2 text-sm text-gray-100 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent';

interface AddRepoForm {
  name: string;
  svn_url: string;
  svn_branch: string;
  svn_username: string;
  svn_password: string;
  git_provider: string;
  git_api_url: string;
  git_repo: string;
  git_branch: string;
  git_token: string;
  sync_mode: string;
  poll_interval_secs: number;
  lfs_threshold_mb: number;
  auto_merge: boolean;
  enabled: boolean;
}

const defaultForm: AddRepoForm = {
  name: '',
  svn_url: '',
  svn_branch: 'trunk',
  svn_username: '',
  svn_password: '',
  git_provider: 'github',
  git_api_url: 'https://api.github.com',
  git_repo: '',
  git_branch: 'main',
  git_token: '',
  sync_mode: 'direct',
  poll_interval_secs: 300,
  lfs_threshold_mb: 0,
  auto_merge: false,
  enabled: true,
};

export default function Repositories() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const user = getStoredUser();
  const isAdmin = user?.role === 'admin';

  const [showAddModal, setShowAddModal] = useState(false);
  const [form, setForm] = useState<AddRepoForm>({ ...defaultForm });
  const [svnTestResult, setSvnTestResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [gitTestResult, setGitTestResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [svnTesting, setSvnTesting] = useState(false);
  const [gitTesting, setGitTesting] = useState(false);

  const { data: repos, isLoading, isError, error } = useQuery({
    queryKey: ['repos'],
    queryFn: api.getRepos,
  });

  const createMutation = useMutation({
    mutationFn: async (data: AddRepoForm) => {
      const { svn_password, git_token, ...repoData } = data;
      const created = await api.createRepo(repoData);
      // Save credentials for the newly created repo
      if (svn_password || git_token) {
        const credData: { svn_password?: string; git_token?: string } = {};
        if (svn_password) credData.svn_password = svn_password;
        if (git_token) credData.git_token = git_token;
        await api.saveRepoCredentials(created.id, credData);
      }
      return created;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['repos'] });
      setShowAddModal(false);
      setForm({ ...defaultForm });
      setSvnTestResult(null);
      setGitTestResult(null);
    },
  });

  function setField<K extends keyof AddRepoForm>(key: K, value: AddRepoForm[K]) {
    setForm((prev) => ({ ...prev, [key]: value }));
  }

  function handleCreate() {
    createMutation.mutate(form);
  }

  async function handleTestSvn() {
    setSvnTesting(true);
    setSvnTestResult(null);
    try {
      const result = await api.testSvnConnection({
        url: form.svn_url,
        username: form.svn_username,
        password: form.svn_password || undefined,
      });
      setSvnTestResult(result);
    } catch (e: any) {
      setSvnTestResult({ ok: false, message: e.message });
    } finally {
      setSvnTesting(false);
    }
  }

  async function handleTestGit() {
    setGitTesting(true);
    setGitTestResult(null);
    try {
      const result = await api.testGitConnection({
        api_url: form.git_api_url,
        repo: form.git_repo,
        provider: form.git_provider,
      });
      setGitTestResult(result);
    } catch (e: any) {
      setGitTestResult({ ok: false, message: e.message });
    } finally {
      setGitTesting(false);
    }
  }

  if (isLoading) {
    return <div className="text-center py-8 text-gray-400">Loading repositories...</div>;
  }

  if (isError) {
    return (
      <div className="text-center py-8 text-red-400">
        Error loading repositories: {error?.message ?? 'Unknown error'}
      </div>
    );
  }

  const repoList = repos ?? [];

  // Separate parents (no parent_id) from children
  const parents = repoList.filter((r) => !r.parent_id);
  const childrenByParent = new Map<string, Repository[]>();
  for (const repo of repoList) {
    if (repo.parent_id) {
      const list = childrenByParent.get(repo.parent_id) ?? [];
      list.push(repo);
      childrenByParent.set(repo.parent_id, list);
    }
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-bold text-gray-100">Repositories</h1>
          <span className="inline-flex items-center justify-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-900/50 text-blue-300">
            {parents.length}
          </span>
        </div>
        {isAdmin && (
          <button
            onClick={() => setShowAddModal(true)}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium transition-colors"
          >
            <Plus className="w-4 h-4" />
            Add Repository
          </button>
        )}
      </div>

      {/* Repo Grid */}
      {parents.length === 0 ? (
        <div className="bg-gray-800/60 border border-gray-700 rounded-lg p-12 text-center">
          <Database className="w-12 h-12 text-gray-600 mx-auto mb-4" />
          <p className="text-gray-400 text-lg">No repositories configured.</p>
          <p className="text-gray-500 text-sm mt-1">
            Click &quot;Add Repository&quot; to get started.
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {parents.map((repo: Repository) => {
            const children = childrenByParent.get(repo.id) ?? [];
            return (
              <div key={repo.id}>
                <button
                  onClick={() => navigate(`/repos/${repo.id}`)}
                  className="w-full bg-gray-800/60 border border-gray-700 rounded-lg p-5 text-left hover:border-blue-500/50 transition-colors group"
                >
                  <div className="flex items-start justify-between mb-3">
                    <h3 className="text-lg font-semibold text-gray-100 group-hover:text-blue-400 transition-colors truncate">
                      {repo.name}
                    </h3>
                    <span
                      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium flex-shrink-0 ml-2 ${
                        repo.enabled
                          ? 'bg-green-900/50 text-green-300'
                          : 'bg-gray-700 text-gray-400'
                      }`}
                    >
                      {repo.enabled ? 'Enabled' : 'Disabled'}
                    </span>
                  </div>

                  <div className="space-y-2 text-sm">
                    <div className="flex items-center gap-2 text-gray-400">
                      <Database className="w-3.5 h-3.5 flex-shrink-0" />
                      <span className="truncate">{repo.svn_url}</span>
                    </div>
                    <div className="flex items-center gap-2 text-gray-400">
                      <GitBranch className="w-3.5 h-3.5 flex-shrink-0" />
                      <span className="truncate">{repo.git_repo}</span>
                    </div>
                    <div className="flex items-center gap-2 text-gray-500 text-xs">
                      <Clock className="w-3.5 h-3.5 flex-shrink-0" />
                      <span>Updated {formatTimeAgo(repo.updated_at)}</span>
                    </div>
                  </div>
                </button>

                {/* Child branch repos */}
                {children.length > 0 && (
                  <div className="ml-4 mt-1 border-l-2 border-gray-700 pl-3 space-y-1">
                    {children.map((child) => (
                      <button
                        key={child.id}
                        onClick={() => navigate(`/repos/${child.id}`)}
                        className="w-full flex items-center justify-between gap-2 px-3 py-2 rounded-md bg-gray-800/40 hover:bg-gray-700/50 transition-colors text-left"
                      >
                        <div className="flex items-center gap-2 min-w-0">
                          <GitBranch className="w-3.5 h-3.5 text-purple-400 flex-shrink-0" />
                          <span className="text-sm text-gray-300 truncate">{child.svn_branch}</span>
                          <span className="text-gray-600 text-xs">/</span>
                          <span className="text-sm text-gray-400 truncate">{child.git_branch}</span>
                        </div>
                        <span
                          className={`w-2 h-2 rounded-full flex-shrink-0 ${
                            child.enabled ? 'bg-green-400' : 'bg-red-400'
                          }`}
                        />
                      </button>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Add Repository Modal */}
      {showAddModal && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50 p-4">
          <div className="bg-gray-800 border border-gray-700 rounded-lg shadow-xl w-full max-w-2xl max-h-[90vh] overflow-y-auto">
            {/* Modal header */}
            <div className="flex items-center justify-between p-6 border-b border-gray-700">
              <h2 className="text-lg font-semibold text-gray-100">Add Repository</h2>
              <button
                onClick={() => { setShowAddModal(false); setForm({ ...defaultForm }); createMutation.reset(); }}
                className="text-gray-400 hover:text-gray-200 transition-colors"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="p-6 space-y-6">
              {createMutation.isError && (
                <div className="bg-red-900/30 border border-red-700 rounded-lg p-3 text-red-300 text-sm">
                  Failed to create repository: {createMutation.error?.message}
                </div>
              )}

              {/* Name */}
              <div>
                <label className="block text-sm font-medium text-gray-300 mb-1">Repository Name</label>
                <input
                  type="text"
                  className={inputClass}
                  value={form.name}
                  onChange={(e) => setField('name', e.target.value)}
                  placeholder="My Project"
                />
              </div>

              {/* SVN Section */}
              <div>
                <h3 className="text-sm font-medium text-gray-300 mb-3 flex items-center gap-2">
                  <Database className="w-4 h-4 text-blue-400" />
                  SVN Configuration
                </h3>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">SVN URL</label>
                    <input
                      type="text"
                      className={inputClass}
                      value={form.svn_url}
                      onChange={(e) => setField('svn_url', e.target.value)}
                      placeholder="https://svn.example.com/repo"
                    />
                  </div>
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">Branch</label>
                    <input
                      type="text"
                      className={inputClass}
                      value={form.svn_branch}
                      onChange={(e) => setField('svn_branch', e.target.value)}
                      placeholder="trunk"
                    />
                  </div>
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">Username</label>
                    <input
                      type="text"
                      className={inputClass}
                      value={form.svn_username}
                      onChange={(e) => setField('svn_username', e.target.value)}
                      placeholder="svn-user"
                    />
                  </div>
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">Password</label>
                    <input
                      type="password"
                      className={inputClass}
                      value={form.svn_password}
                      onChange={(e) => setField('svn_password', e.target.value)}
                      placeholder="Enter SVN password"
                    />
                  </div>
                  <div className="md:col-span-2">
                    <button
                      type="button"
                      onClick={handleTestSvn}
                      disabled={svnTesting || !form.svn_url}
                      className="inline-flex items-center gap-2 px-3 py-1.5 rounded-md text-xs font-medium border border-blue-600 text-blue-300 hover:bg-blue-900/30 disabled:opacity-50 transition-colors"
                    >
                      {svnTesting ? 'Testing...' : 'Test SVN Connection'}
                    </button>
                    {svnTestResult && (
                      <span className={`ml-3 text-xs ${svnTestResult.ok ? 'text-green-300' : 'text-red-300'}`}>
                        {svnTestResult.message}
                      </span>
                    )}
                  </div>
                </div>
              </div>

              {/* Git Section */}
              <div>
                <h3 className="text-sm font-medium text-gray-300 mb-3 flex items-center gap-2">
                  <GitBranch className="w-4 h-4 text-purple-400" />
                  Git Configuration
                </h3>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">Provider</label>
                    <select
                      className={selectClass}
                      value={form.git_provider}
                      onChange={(e) => setField('git_provider', e.target.value)}
                    >
                      <option value="github">GitHub</option>
                      <option value="gitea">Gitea</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">API URL</label>
                    <input
                      type="text"
                      className={inputClass}
                      value={form.git_api_url}
                      onChange={(e) => setField('git_api_url', e.target.value)}
                      placeholder="https://api.github.com"
                    />
                  </div>
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">Repository</label>
                    <input
                      type="text"
                      className={inputClass}
                      value={form.git_repo}
                      onChange={(e) => setField('git_repo', e.target.value)}
                      placeholder="owner/repo"
                    />
                  </div>
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">Default Branch</label>
                    <input
                      type="text"
                      className={inputClass}
                      value={form.git_branch}
                      onChange={(e) => setField('git_branch', e.target.value)}
                      placeholder="main"
                    />
                  </div>
                  <div className="md:col-span-2">
                    <label className="block text-sm text-gray-400 mb-1">Git Token</label>
                    <input
                      type="password"
                      className={inputClass}
                      value={form.git_token}
                      onChange={(e) => setField('git_token', e.target.value)}
                      placeholder="Enter Git API token"
                    />
                  </div>
                  <div className="md:col-span-2">
                    <button
                      type="button"
                      onClick={handleTestGit}
                      disabled={gitTesting || !form.git_repo}
                      className="inline-flex items-center gap-2 px-3 py-1.5 rounded-md text-xs font-medium border border-purple-600 text-purple-300 hover:bg-purple-900/30 disabled:opacity-50 transition-colors"
                    >
                      {gitTesting ? 'Testing...' : 'Test Git Connection'}
                    </button>
                    {gitTestResult && (
                      <span className={`ml-3 text-xs ${gitTestResult.ok ? 'text-green-300' : 'text-red-300'}`}>
                        {gitTestResult.message}
                      </span>
                    )}
                  </div>
                </div>
              </div>

              {/* Sync Section */}
              <div>
                <h3 className="text-sm font-medium text-gray-300 mb-3 flex items-center gap-2">
                  <Clock className="w-4 h-4 text-green-400" />
                  Sync Settings
                </h3>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">Sync Mode</label>
                    <select
                      className={selectClass}
                      value={form.sync_mode}
                      onChange={(e) => setField('sync_mode', e.target.value)}
                    >
                      <option value="direct">Direct</option>
                      <option value="pr">Pull Request</option>
                    </select>
                  </div>
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">Poll Interval (seconds)</label>
                    <input
                      type="number"
                      className={inputClass}
                      value={form.poll_interval_secs}
                      onChange={(e) => setField('poll_interval_secs', Number(e.target.value))}
                      min={10}
                    />
                  </div>
                  <div>
                    <label className="block text-sm text-gray-400 mb-1">LFS Threshold (MB, 0 = disabled)</label>
                    <input
                      type="number"
                      className={inputClass}
                      value={form.lfs_threshold_mb}
                      onChange={(e) => setField('lfs_threshold_mb', Number(e.target.value))}
                      min={0}
                    />
                  </div>
                  <div className="flex items-end gap-3">
                    <div>
                      <label className="block text-sm text-gray-400 mb-1">Auto Merge</label>
                      <button
                        type="button"
                        onClick={() => setField('auto_merge', !form.auto_merge)}
                        className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                          form.auto_merge ? 'bg-blue-600' : 'bg-gray-600'
                        }`}
                      >
                        <span
                          className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                            form.auto_merge ? 'translate-x-6' : 'translate-x-1'
                          }`}
                        />
                      </button>
                    </div>
                    <div>
                      <label className="block text-sm text-gray-400 mb-1">Enabled</label>
                      <button
                        type="button"
                        onClick={() => setField('enabled', !form.enabled)}
                        className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                          form.enabled ? 'bg-green-600' : 'bg-gray-600'
                        }`}
                      >
                        <span
                          className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                            form.enabled ? 'translate-x-6' : 'translate-x-1'
                          }`}
                        />
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </div>

            {/* Modal footer */}
            <div className="flex items-center justify-end gap-3 p-6 border-t border-gray-700">
              <button
                onClick={() => { setShowAddModal(false); setForm({ ...defaultForm }); createMutation.reset(); }}
                className="px-4 py-2 rounded-lg border border-gray-600 text-gray-300 hover:text-white text-sm font-medium transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleCreate}
                disabled={createMutation.isPending || !form.name.trim()}
                className="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 disabled:opacity-50 text-white text-sm font-medium transition-colors"
              >
                <Plus className="w-4 h-4" />
                {createMutation.isPending ? 'Creating...' : 'Create Repository'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
