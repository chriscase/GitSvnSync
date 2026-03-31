import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import { api, type AuditEntry, type SyncRecord, type CommitMapEntry, type Repository } from '../api';
import ImportProgressCard from '../components/ImportProgressCard';
import ServerMonitor from '../components/ServerMonitor';

export default function Dashboard() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [selectedRepoId, setSelectedRepoId] = useState<string>('all');
  const [expandedGroups, setExpandedGroups] = useState<Set<number>>(new Set());

  const activeRepoId = selectedRepoId !== 'all' ? selectedRepoId : undefined;

  const { data: status, isLoading: statusLoading, isError, error } = useQuery({
    queryKey: ['status'],
    queryFn: api.getStatus,
    refetchInterval: 5000,
  });

  const { data: recentActivity } = useQuery({
    queryKey: ['audit', 'recent', activeRepoId],
    queryFn: () => api.getAuditLog(20, undefined, undefined, activeRepoId),
  });

  const { data: syncRecords } = useQuery({
    queryKey: ['sync-records', activeRepoId],
    queryFn: () => api.getSyncRecords(20, activeRepoId),
  });

  const { data: commitMap } = useQuery({
    queryKey: ['commit-map', activeRepoId],
    queryFn: () => api.getCommitMap(15, activeRepoId),
  });

  const { data: repos } = useQuery({
    queryKey: ['repos'],
    queryFn: api.getRepos,
  });

  if (statusLoading) {
    return <div className="text-center py-8 text-gray-400">Loading...</div>;
  }

  if (isError) {
    return (
      <div className="text-center py-8 text-red-400">
        Error loading status: {error?.message ?? 'Unknown error'}
      </div>
    );
  }

  const formatUptime = (secs: number) => {
    const days = Math.floor(secs / 86400);
    const hours = Math.floor((secs % 86400) / 3600);
    const mins = Math.floor((secs % 3600) / 60);
    if (days > 0) return `${days}d ${hours}h ${mins}m`;
    if (hours > 0) return `${hours}h ${mins}m`;
    return `${mins}m`;
  };

  const formatTimeAgo = (isoDate: string): string => {
    const diff = Math.max(0, Math.floor((Date.now() - new Date(isoDate).getTime()) / 1000));
    if (diff < 60) return `${diff}s ago`;
    const mins = Math.floor(diff / 60);
    if (mins < 60) return `${mins}m ago`;
    const hrs = Math.floor(mins / 60);
    if (hrs < 24) return `${hrs}h ${mins % 60}m ago`;
    return `${Math.floor(hrs / 24)}d ago`;
  };

  const selectedRepo = selectedRepoId !== 'all'
    ? repos?.find(r => r.id === selectedRepoId)
    : null;

  const repoContextLabel = selectedRepo ? selectedRepo.name : 'All Repositories';

  const repoName = repos && repos.length === 1 ? repos[0].name : (repos && repos.length > 1 ? 'All' : 'Default');

  const entries = recentActivity?.entries ?? [];
  const records = syncRecords?.entries ?? [];
  const cmEntries = commitMap?.entries ?? [];

  // Group consecutive audit entries with same (action, success) for collapsed display
  function groupEntries(items: AuditEntry[]): { key: number; entries: AuditEntry[] }[] {
    const groups: { key: number; entries: AuditEntry[] }[] = [];
    for (const entry of items) {
      const last = groups[groups.length - 1];
      if (last && last.entries[0].action === entry.action && last.entries[0].success === entry.success) {
        last.entries.push(entry);
      } else {
        groups.push({ key: entry.id, entries: [entry] });
      }
    }
    return groups;
  }
  const auditGroups = groupEntries(entries);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-100">Dashboard</h1>
        {repos && repos.length > 1 && (
          <select
            value={selectedRepoId}
            onChange={(e) => setSelectedRepoId(e.target.value)}
            className="bg-gray-800 border border-gray-600 text-gray-200 rounded-md px-3 py-1.5 text-sm"
          >
            <option value="all">All Repositories</option>
            {repos.map(r => (
              <option key={r.id} value={r.id}>{r.name}</option>
            ))}
          </select>
        )}
      </div>

      {/* Import Progress Card */}
      <ImportProgressCard />

      {/* Repositories Overview */}
      {repos && repos.length > 0 && (
        <div className="bg-gray-800/60 border border-gray-700 rounded-lg p-5">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-semibold text-gray-100">Repositories</h2>
            <button
              onClick={() => navigate('/repos')}
              className="text-sm text-blue-400 hover:text-blue-300 transition-colors"
            >
              Manage Repositories &rarr;
            </button>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
            {repos.map((repo: Repository) => (
              <button
                key={repo.id}
                onClick={() => navigate(`/repos/${repo.id}`)}
                className="bg-gray-900/60 border border-gray-700 rounded-lg p-3 text-left hover:border-blue-500/50 transition-colors"
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="text-sm font-semibold text-gray-200 truncate">{repo.name}</span>
                  <span
                    className={`inline-block w-2 h-2 rounded-full flex-shrink-0 ml-2 ${
                      repo.enabled ? 'bg-green-400' : 'bg-gray-500'
                    }`}
                    title={repo.enabled ? 'Enabled' : 'Disabled'}
                  />
                </div>
                <p className="text-xs text-gray-500 truncate">{repo.git_repo}</p>
                <p className="text-xs text-gray-600 mt-1">
                  Updated {formatTimeAgo(repo.updated_at)}
                </p>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Status Cards */}
      <div className="mb-1">
        <span className="text-xs text-blue-400">{repoContextLabel}</span>
      </div>
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
        <StatusCard
          title="Sync State"
          value={status?.state ?? 'unknown'}
          color={
            status?.state === 'idle'
              ? 'green'
              : status?.state === 'error'
                ? 'red'
                : 'yellow'
          }
        />
        <StatusCard
          title="Total Syncs"
          value={String(status?.total_syncs ?? 0)}
          color="blue"
          onClick={() => navigate('/audit')}
        />
        <StatusCard
          title="Active Conflicts"
          value={String(status?.active_conflicts ?? 0)}
          color={status?.active_conflicts ? 'red' : 'green'}
          onClick={() => navigate('/conflicts')}
        />
        <StatusCard
          title="Errors (24h)"
          value={String(status?.total_errors ?? 0)}
          color={status?.total_errors ? 'red' : 'gray'}
          onClick={() => navigate('/audit?success=false')}
          subtitle={
            status?.last_error_at
              ? `Last: ${formatTimeAgo(status.last_error_at)}`
              : 'No recent errors'
          }
          onClear={
            (status?.total_errors ?? 0) > 0
              ? async () => {
                  await api.resetErrors();
                  queryClient.invalidateQueries({ queryKey: ['status'] });
                }
              : undefined
          }
        />
        <StatusCard
          title="Uptime"
          value={status ? formatUptime(status.uptime_secs) : '-'}
          color="gray"
        />
        <StatusCard
          title="Last Sync"
          value={status?.last_sync_at ? formatTimeAgo(status.last_sync_at) : 'Never'}
          color="gray"
        />
      </div>

      {/* Sync Position */}
      <div className="bg-gray-800 shadow rounded-lg p-6 border border-gray-700">
        <h2 className="text-lg font-semibold text-gray-100 mb-4">
          Sync Position
          {selectedRepo && (
            <span className="ml-2 text-sm font-normal text-blue-400">— {selectedRepo.name}</span>
          )}
        </h2>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <div>
            <span className="text-sm text-gray-400">Last SVN Revision</span>
            <p className="text-lg font-mono text-gray-100">
              {status?.last_svn_revision != null
                ? `r${status.last_svn_revision}`
                : 'Not synced'}
            </p>
          </div>
          <div>
            <span className="text-sm text-gray-400">Last Git Hash</span>
            <p className="text-lg font-mono truncate text-gray-100">
              {status?.last_git_hash
                ? status.last_git_hash.substring(0, 12)
                : 'Not synced'}
            </p>
          </div>
          <div>
            <span className="text-sm text-gray-400">Last Sync</span>
            <p className="text-lg text-gray-100">
              {status?.last_sync_at
                ? new Date(status.last_sync_at).toLocaleString()
                : 'Never'}
            </p>
          </div>
          <div>
            <span className="text-sm text-gray-400">Total Conflicts</span>
            <p className="text-lg text-gray-100">
              {status?.total_conflicts ?? 0}
            </p>
          </div>
        </div>
      </div>

      {/* Sync Records with expandable diffs */}
      <div className="bg-gray-800 shadow rounded-lg border border-gray-700">
        <div className="p-6 pb-3">
          <h2 className="text-lg font-semibold text-gray-100">
            Recent Sync Records
            {selectedRepo && (
              <span className="ml-2 text-sm font-normal text-blue-400">— {selectedRepo.name}</span>
            )}
          </h2>
          <p className="text-sm text-gray-400 mt-1">
            Individual commits synced between SVN and Git (click to expand)
          </p>
          {!selectedRepo && repos && repos.length > 1 && (
            <p className="text-xs text-gray-500 mt-1">
              Showing data from all repositories. Select a specific repository to filter.
            </p>
          )}
        </div>
        {records.length > 0 ? (
          <div className="divide-y divide-gray-700">
            {records.map((record) => (
              <SyncRecordRow key={record.id} record={record} repoName={repoName} />
            ))}
          </div>
        ) : (
          <p className="text-gray-400 text-sm px-6 pb-6">No sync records yet</p>
        )}
      </div>

      {/* Commit Map */}
      <div className="bg-gray-800 shadow rounded-lg border border-gray-700">
        <div className="p-6 pb-3">
          <h2 className="text-lg font-semibold text-gray-100">
            Commit Map (SVN &harr; Git)
            {selectedRepo && (
              <span className="ml-2 text-sm font-normal text-blue-400">— {selectedRepo.name}</span>
            )}
          </h2>
          <p className="text-sm text-gray-400 mt-1">
            Bidirectional mapping between SVN revisions and Git commits
          </p>
        </div>
        {cmEntries.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="min-w-full divide-y divide-gray-700">
              <thead>
                <tr>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-400 uppercase">Repository</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-400 uppercase">SVN Rev</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-400 uppercase">Git SHA</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-400 uppercase">Direction</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-400 uppercase">SVN Author</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-400 uppercase">Git Author</th>
                  <th className="px-6 py-3 text-left text-xs font-medium text-gray-400 uppercase">Synced At</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-700">
                {cmEntries.map((cm: CommitMapEntry) => (
                  <tr key={cm.id} className="hover:bg-gray-700/50">
                    <td className="px-6 py-3">
                      <RepoBadge name={repoName} />
                    </td>
                    <td className="px-6 py-3 text-sm font-mono text-blue-400">r{cm.svn_rev}</td>
                    <td className="px-6 py-3 text-sm font-mono text-purple-400 truncate max-w-[200px]">
                      {cm.git_sha.substring(0, 12)}
                    </td>
                    <td className="px-6 py-3">
                      <DirectionBadge direction={cm.direction} />
                    </td>
                    <td className="px-6 py-3 text-sm text-gray-300">{cm.svn_author}</td>
                    <td className="px-6 py-3 text-sm text-gray-300 truncate max-w-[200px]">{cm.git_author}</td>
                    <td className="px-6 py-3 text-sm text-gray-400">
                      {new Date(cm.synced_at).toLocaleString()}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <p className="text-gray-400 text-sm px-6 pb-6">No commit mappings yet</p>
        )}
      </div>

      {/* Recent Activity */}
      <div className="bg-gray-800 shadow rounded-lg p-6 border border-gray-700">
        <h2 className="text-lg font-semibold text-gray-100 mb-4">
          Recent Activity
          {selectedRepo && (
            <span className="ml-2 text-sm font-normal text-blue-400">— {selectedRepo.name}</span>
          )}
        </h2>
        {auditGroups.length > 0 ? (
          <div className="space-y-1">
            {auditGroups.map((group) => {
              const latest = group.entries[0];
              const isGroup = group.entries.length > 1;
              const isExpanded = expandedGroups.has(group.key);
              return (
                <div key={group.key}>
                  <div
                    className={`flex items-center justify-between py-2 border-b border-gray-700 last:border-0 ${isGroup ? 'cursor-pointer hover:bg-gray-700/30' : ''}`}
                    onClick={() => {
                      if (!isGroup) return;
                      setExpandedGroups(prev => {
                        const next = new Set(prev);
                        if (next.has(group.key)) next.delete(group.key); else next.add(group.key);
                        return next;
                      });
                    }}
                  >
                    <div className="flex items-center space-x-3">
                      {isGroup && (
                        <span className="text-gray-500 w-4">{isExpanded ? '\u25BE' : '\u25B8'}</span>
                      )}
                      <RepoBadge name={repoName} />
                      <SuccessIndicator success={latest.success} />
                      {latest.direction && <DirectionBadge direction={latest.direction} />}
                      <ActionBadge action={latest.action} />
                      {isGroup && (
                        <span className="text-xs bg-gray-600 text-gray-300 rounded-full px-2 py-0.5">
                          &times;{group.entries.length}
                        </span>
                      )}
                      <span className="text-sm text-gray-200 truncate max-w-xl lg:max-w-2xl" title={latest.details || latest.action}>
                        {latest.details || latest.action}
                      </span>
                      {latest.author && (
                        <span className="text-sm text-gray-400">by {latest.author}</span>
                      )}
                    </div>
                    <div className="flex items-center space-x-3 flex-shrink-0">
                      {latest.svn_rev && (
                        <span className="text-xs font-mono text-blue-400">r{latest.svn_rev}</span>
                      )}
                      {latest.git_sha && (
                        <span className="text-xs font-mono text-purple-400">
                          {latest.git_sha.substring(0, 8)}
                        </span>
                      )}
                      <span className="text-xs text-gray-500">
                        {new Date(latest.created_at).toLocaleString()}
                      </span>
                    </div>
                  </div>
                  {isGroup && isExpanded && (
                    <div className="ml-8 border-l-2 border-gray-700 pl-4 space-y-1">
                      {group.entries.slice(1).map((entry: AuditEntry) => (
                        <div key={entry.id} className="flex items-center justify-between py-1.5 text-sm text-gray-400">
                          <span className="truncate max-w-xl">{entry.details || entry.action}</span>
                          <span className="text-xs text-gray-500 flex-shrink-0 ml-3">
                            {new Date(entry.created_at).toLocaleString()}
                          </span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        ) : (
          <p className="text-gray-400 text-sm">No activity yet</p>
        )}
      </div>

      {/* Server Monitor */}
      <ServerMonitor />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function SyncRecordRow({ record, repoName }: { record: SyncRecord; repoName: string }) {
  const [expanded, setExpanded] = useState(false);

  const statusColor =
    record.status === 'applied'
      ? 'text-green-400'
      : record.status === 'failed'
        ? 'text-red-400'
        : 'text-yellow-400';

  return (
    <div>
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full px-6 py-3 flex items-center justify-between hover:bg-gray-700/50 text-left transition-colors"
      >
        <div className="flex items-center space-x-3 min-w-0">
          <RepoBadge name={repoName} />
          <span className={`text-xs font-bold uppercase ${statusColor}`}>
            {record.status === 'applied' ? '\u2713' : record.status === 'failed' ? '\u2717' : '\u25CB'}
          </span>
          <DirectionBadge direction={record.direction} />
          <span className="text-sm text-gray-200 truncate">{record.message}</span>
        </div>
        <div className="flex items-center space-x-3 flex-shrink-0 ml-4">
          <span className="text-sm text-gray-400">{record.author}</span>
          {record.svn_rev && (
            <span className="text-xs font-mono text-blue-400">r{record.svn_rev}</span>
          )}
          {record.git_sha && (
            <span className="text-xs font-mono text-purple-400">{record.git_sha.substring(0, 8)}</span>
          )}
          <span className="text-xs text-gray-500">
            {new Date(record.synced_at).toLocaleString()}
          </span>
          <svg
            className={`w-4 h-4 text-gray-400 transition-transform ${expanded ? 'rotate-180' : ''}`}
            fill="none" viewBox="0 0 24 24" stroke="currentColor"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
          </svg>
        </div>
      </button>
      {expanded && (
        <div className="px-6 pb-4 bg-gray-850">
          <div className="bg-gray-900 rounded-lg p-4 border border-gray-700">
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-4 text-sm">
              <div>
                <span className="text-gray-500 text-xs uppercase">Record ID</span>
                <p className="font-mono text-gray-300 truncate">{record.id}</p>
              </div>
              <div>
                <span className="text-gray-500 text-xs uppercase">SVN Revision</span>
                <p className="font-mono text-blue-400">{record.svn_rev ? `r${record.svn_rev}` : 'N/A'}</p>
              </div>
              <div>
                <span className="text-gray-500 text-xs uppercase">Git SHA</span>
                <p className="font-mono text-purple-400">{record.git_sha || 'N/A'}</p>
              </div>
              <div>
                <span className="text-gray-500 text-xs uppercase">Status</span>
                <p className={statusColor + ' font-medium capitalize'}>{record.status}</p>
              </div>
            </div>
            <div className="grid grid-cols-2 md:grid-cols-3 gap-4 text-sm">
              <div>
                <span className="text-gray-500 text-xs uppercase">Author</span>
                <p className="text-gray-300">{record.author}</p>
              </div>
              <div>
                <span className="text-gray-500 text-xs uppercase">Committed</span>
                <p className="text-gray-300">{new Date(record.timestamp).toLocaleString()}</p>
              </div>
              <div>
                <span className="text-gray-500 text-xs uppercase">Synced At</span>
                <p className="text-gray-300">{new Date(record.synced_at).toLocaleString()}</p>
              </div>
            </div>
            <div className="mt-4">
              <span className="text-gray-500 text-xs uppercase">Commit Message</span>
              <div className="mt-1 bg-gray-800 rounded p-3 border border-gray-700">
                <pre className="text-sm text-gray-200 whitespace-pre-wrap font-mono">{record.message}</pre>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function StatusCard({
  title,
  value,
  color,
  onClick,
  subtitle,
  onClear,
}: {
  title: string;
  value: string;
  color: string;
  onClick?: () => void;
  subtitle?: string;
  onClear?: () => void;
}) {
  const colorClasses: Record<string, string> = {
    green: 'bg-green-900/30 border-green-700',
    red: 'bg-red-900/30 border-red-700',
    yellow: 'bg-yellow-900/30 border-yellow-700',
    blue: 'bg-blue-900/30 border-blue-700',
    gray: 'bg-gray-800 border-gray-700',
  };

  const clickableClasses = onClick
    ? 'cursor-pointer hover:border-blue-500/50 transition-colors'
    : '';

  return (
    <div
      className={`rounded-lg border p-4 ${colorClasses[color] ?? colorClasses.gray} ${clickableClasses}`}
      onClick={onClick}
    >
      <div className="flex items-center justify-between">
        <p className="text-sm text-gray-400">{title}</p>
        {onClear && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onClear();
            }}
            className="text-xs text-gray-400 hover:text-red-400 transition-colors px-1.5 py-0.5 rounded border border-gray-600 hover:border-red-500/50"
          >
            Clear
          </button>
        )}
      </div>
      <p className="text-2xl font-bold capitalize text-gray-100">{value}</p>
      {subtitle && <p className="text-xs text-gray-500 mt-1">{subtitle}</p>}
    </div>
  );
}

function RepoBadge({ name }: { name: string }) {
  return (
    <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-blue-900/40 text-blue-300 border border-blue-700/30 truncate max-w-[120px]">
      {name}
    </span>
  );
}

function DirectionBadge({ direction }: { direction: string }) {
  const isToGit = direction === 'svn_to_git';
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
        isToGit
          ? 'bg-blue-900/50 text-blue-300'
          : 'bg-purple-900/50 text-purple-300'
      }`}
    >
      {isToGit ? 'SVN \u2192 Git' : 'Git \u2192 SVN'}
    </span>
  );
}

function ActionBadge({ action }: { action: string }) {
  const colors: Record<string, string> = {
    sync_cycle: 'bg-cyan-900/50 text-cyan-300',
    conflict_detected: 'bg-red-900/50 text-red-300',
    conflict_resolved: 'bg-green-900/50 text-green-300',
    sync_error: 'bg-red-900/50 text-red-300',
    webhook_received: 'bg-yellow-900/50 text-yellow-300',
    daemon_started: 'bg-emerald-900/50 text-emerald-300',
    auth_login: 'bg-indigo-900/50 text-indigo-300',
    config_updated: 'bg-orange-900/50 text-orange-300',
  };

  const label = action.replace(/_/g, ' ');

  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
        colors[action] ?? 'bg-gray-700 text-gray-300'
      }`}
    >
      {label}
    </span>
  );
}

function SuccessIndicator({ success }: { success: boolean }) {
  return (
    <span
      className={`inline-block w-2 h-2 rounded-full flex-shrink-0 ${
        success ? 'bg-green-400' : 'bg-red-400'
      }`}
      title={success ? 'Success' : 'Failed'}
    />
  );
}
