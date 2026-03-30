import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { useParams, useNavigate, Link } from 'react-router-dom';
import { api } from '../api';
import { ArrowLeft, RefreshCw, Settings, Database, GitBranch, Clock, Activity, Zap } from 'lucide-react';

function formatTimeAgo(isoDate: string): string {
  const diff = Math.max(0, Math.floor((Date.now() - new Date(isoDate).getTime()) / 1000));
  if (diff < 60) return `${diff}s ago`;
  const mins = Math.floor(diff / 60);
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ${mins % 60}m ago`;
  return `${Math.floor(hrs / 24)}d ago`;
}

export default function RepoDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [syncTriggered, setSyncTriggered] = useState(false);

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
          <button
            onClick={() => navigate(`/setup`)}
            className="inline-flex items-center gap-2 px-4 py-2 rounded-lg border border-gray-600 hover:border-gray-500 text-gray-300 hover:text-white text-sm font-medium transition-colors"
          >
            <Settings className="w-4 h-4" />
            Edit
          </button>
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

      {syncMutation.isError && (
        <div className="bg-red-900/30 border border-red-700 rounded-lg p-4 text-red-300 text-sm">
          Failed to trigger sync: {syncMutation.error?.message}
        </div>
      )}

      {/* Config Summary - SVN and Git side by side */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {/* SVN Config */}
        <div className="bg-gray-800/60 border border-gray-700 rounded-lg p-5">
          <div className="flex items-center gap-2 mb-4">
            <Database className="w-5 h-5 text-blue-400" />
            <h2 className="text-lg font-semibold text-gray-100">SVN Configuration</h2>
          </div>
          <div className="space-y-3">
            <ConfigRow label="URL" value={repo.svn_url} />
            <ConfigRow label="Branch" value={repo.svn_branch} />
            <ConfigRow label="Username" value={repo.svn_username} />
          </div>
        </div>

        {/* Git Config */}
        <div className="bg-gray-800/60 border border-gray-700 rounded-lg p-5">
          <div className="flex items-center gap-2 mb-4">
            <GitBranch className="w-5 h-5 text-purple-400" />
            <h2 className="text-lg font-semibold text-gray-100">Git Configuration</h2>
          </div>
          <div className="space-y-3">
            <ConfigRow label="Provider" value={repo.git_provider} />
            <ConfigRow label="API URL" value={repo.git_api_url} />
            <ConfigRow label="Repository" value={repo.git_repo} />
            <ConfigRow label="Branch" value={repo.git_branch} />
          </div>
        </div>
      </div>

      {/* Sync Status */}
      <div className="bg-gray-800/60 border border-gray-700 rounded-lg p-5">
        <div className="flex items-center gap-2 mb-4">
          <Activity className="w-5 h-5 text-green-400" />
          <h2 className="text-lg font-semibold text-gray-100">Sync Status</h2>
        </div>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <div>
            <span className="text-sm text-gray-400">Last Updated</span>
            <p className="text-lg text-gray-100">
              {formatTimeAgo(repo.updated_at)}
            </p>
          </div>
          <div>
            <span className="text-sm text-gray-400">Sync Mode</span>
            <p className="text-lg text-gray-100 capitalize">{repo.sync_mode}</p>
          </div>
          <div>
            <span className="text-sm text-gray-400">Poll Interval</span>
            <p className="text-lg text-gray-100">{repo.poll_interval_secs}s</p>
          </div>
          <div>
            <span className="text-sm text-gray-400">Auto Merge</span>
            <p className="text-lg text-gray-100">{repo.auto_merge ? 'Yes' : 'No'}</p>
          </div>
        </div>
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
          label="Sync Mode"
          value={repo.sync_mode}
        />
        <StatCard
          icon={<Database className="w-5 h-5 text-purple-400" />}
          label="Created By"
          value={repo.created_by ?? 'System'}
        />
      </div>
    </div>
  );
}

function ConfigRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline justify-between gap-4">
      <span className="text-sm text-gray-400 flex-shrink-0">{label}</span>
      <span className="text-sm text-gray-200 truncate text-right font-mono">{value}</span>
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
