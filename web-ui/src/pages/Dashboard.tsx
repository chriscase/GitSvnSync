import { useQuery } from '@tanstack/react-query';
import { api, type AuditEntry } from '../api';

export default function Dashboard() {
  const { data: status, isLoading: statusLoading, isError, error } = useQuery({
    queryKey: ['status'],
    queryFn: api.getStatus,
  });

  const { data: recentActivity } = useQuery({
    queryKey: ['audit', 'recent'],
    queryFn: () => api.getAuditLog(20),
  });

  if (statusLoading) {
    return <div className="text-center py-8 text-gray-500">Loading...</div>;
  }

  if (isError) {
    return (
      <div className="text-center py-8 text-red-500">
        Error loading status: {error?.message ?? 'Unknown error'}
      </div>
    );
  }

  const formatUptime = (secs: number) => {
    const days = Math.floor(secs / 86400);
    const hours = Math.floor((secs % 86400) / 3600);
    const mins = Math.floor((secs % 3600) / 60);
    if (days > 0) return `${days}d ${hours}h`;
    if (hours > 0) return `${hours}h ${mins}m`;
    return `${mins}m`;
  };

  const entries = recentActivity?.entries ?? [];

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-gray-900">Dashboard</h1>

      {/* Status Cards */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
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
        />
        <StatusCard
          title="Active Conflicts"
          value={String(status?.active_conflicts ?? 0)}
          color={status?.active_conflicts ? 'red' : 'green'}
        />
        <StatusCard
          title="Uptime"
          value={status ? formatUptime(status.uptime_secs) : '-'}
          color="gray"
        />
      </div>

      {/* Watermarks */}
      <div className="bg-white shadow rounded-lg p-6">
        <h2 className="text-lg font-semibold text-gray-900 mb-4">
          Sync Position
        </h2>
        <div className="grid grid-cols-2 gap-4">
          <div>
            <span className="text-sm text-gray-500">Last SVN Revision</span>
            <p className="text-lg font-mono">
              {status?.last_svn_revision != null
                ? `r${status.last_svn_revision}`
                : 'Not synced'}
            </p>
          </div>
          <div>
            <span className="text-sm text-gray-500">Last Git Hash</span>
            <p className="text-lg font-mono truncate">
              {status?.last_git_hash
                ? status.last_git_hash.substring(0, 12)
                : 'Not synced'}
            </p>
          </div>
        </div>
      </div>

      {/* Recent Activity */}
      <div className="bg-white shadow rounded-lg p-6">
        <h2 className="text-lg font-semibold text-gray-900 mb-4">
          Recent Activity
        </h2>
        {entries.length > 0 ? (
          <div className="space-y-2">
            {entries.map((entry: AuditEntry) => (
              <div
                key={entry.id}
                className="flex items-center justify-between py-2 border-b border-gray-100 last:border-0"
              >
                <div className="flex items-center space-x-3">
                  {entry.direction && (
                    <DirectionBadge direction={entry.direction} />
                  )}
                  <span className="text-sm text-gray-900">{entry.action}</span>
                  {entry.author && (
                    <span className="text-sm text-gray-500">{entry.author}</span>
                  )}
                </div>
                <span className="text-xs text-gray-400">
                  {new Date(entry.created_at).toLocaleString()}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-gray-500 text-sm">No activity yet</p>
        )}
      </div>
    </div>
  );
}

function StatusCard({
  title,
  value,
  color,
}: {
  title: string;
  value: string;
  color: string;
}) {
  const colorClasses: Record<string, string> = {
    green: 'bg-green-50 border-green-200',
    red: 'bg-red-50 border-red-200',
    yellow: 'bg-yellow-50 border-yellow-200',
    blue: 'bg-blue-50 border-blue-200',
    gray: 'bg-gray-50 border-gray-200',
  };

  return (
    <div className={`rounded-lg border p-4 ${colorClasses[color] ?? colorClasses.gray}`}>
      <p className="text-sm text-gray-600">{title}</p>
      <p className="text-2xl font-bold capitalize">{value}</p>
    </div>
  );
}

function DirectionBadge({ direction }: { direction: string }) {
  const isToGit = direction === 'svn_to_git';
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
        isToGit
          ? 'bg-blue-100 text-blue-800'
          : 'bg-purple-100 text-purple-800'
      }`}
    >
      {isToGit ? 'SVN → Git' : 'Git → SVN'}
    </span>
  );
}
