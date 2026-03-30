import { useQuery } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { api, type Repository, type User } from '../api';
import { GitBranch, Plus, Database, Clock } from 'lucide-react';

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

export default function Repositories() {
  const navigate = useNavigate();
  const user = getStoredUser();
  const isAdmin = user?.role === 'admin';

  const { data: repos, isLoading, isError, error } = useQuery({
    queryKey: ['repos'],
    queryFn: api.getRepos,
  });

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

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-bold text-gray-100">Repositories</h1>
          <span className="inline-flex items-center justify-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-blue-900/50 text-blue-300">
            {repoList.length}
          </span>
        </div>
        {isAdmin && (
          <button
            onClick={() => navigate('/setup')}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium transition-colors"
          >
            <Plus className="w-4 h-4" />
            Add Repository
          </button>
        )}
      </div>

      {/* Repo Grid */}
      {repoList.length === 0 ? (
        <div className="bg-gray-800/60 border border-gray-700 rounded-lg p-12 text-center">
          <Database className="w-12 h-12 text-gray-600 mx-auto mb-4" />
          <p className="text-gray-400 text-lg">No repositories configured.</p>
          <p className="text-gray-500 text-sm mt-1">
            Click "Add Repository" to get started.
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
          {repoList.map((repo: Repository) => (
            <button
              key={repo.id}
              onClick={() => navigate(`/repos/${repo.id}`)}
              className="bg-gray-800/60 border border-gray-700 rounded-lg p-5 text-left hover:border-blue-500/50 transition-colors group"
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
          ))}
        </div>
      )}
    </div>
  );
}
