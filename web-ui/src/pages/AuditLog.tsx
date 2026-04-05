import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { api, type AuditEntry } from '../api';

export default function AuditLog() {
  const [limit] = useState(50);

  const { data: response, isLoading, isError, error } = useQuery({
    queryKey: ['audit', limit],
    queryFn: () => api.getAuditLog(limit),
  });

  if (isLoading) {
    return <div className="text-center py-8 text-gray-500">Loading...</div>;
  }

  if (isError) {
    return (
      <div className="text-center py-8 text-red-500">
        Error loading audit log: {error?.message ?? 'Unknown error'}
      </div>
    );
  }

  const entries = response?.entries ?? [];

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-gray-900">Audit Log</h1>

      <div className="bg-white shadow overflow-hidden rounded-lg">
        <table className="min-w-full divide-y divide-gray-200">
          <thead className="bg-gray-50">
            <tr>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                Time
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                Direction
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                Action
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                Author
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                SVN Rev
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                Git SHA
              </th>
              <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase">
                Details
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-200">
            {entries.map((entry: AuditEntry) => (
              <tr key={entry.id} className="hover:bg-gray-50">
                <td className="px-6 py-3 text-sm text-gray-500 whitespace-nowrap">
                  {new Date(entry.created_at).toLocaleString()}
                </td>
                <td className="px-6 py-3 whitespace-nowrap">
                  {entry.direction ? (
                    <DirectionBadge direction={entry.direction} />
                  ) : (
                    <span className="text-gray-400">-</span>
                  )}
                </td>
                <td className="px-6 py-3 text-sm text-gray-900">
                  {entry.action}
                </td>
                <td className="px-6 py-3 text-sm text-gray-700">
                  {entry.author ?? '-'}
                </td>
                <td className="px-6 py-3 text-sm font-mono text-gray-500">
                  {entry.svn_rev ?? '-'}
                </td>
                <td className="px-6 py-3 text-sm font-mono text-gray-500 truncate max-w-[120px]">
                  {entry.git_sha ? entry.git_sha.substring(0, 8) : '-'}
                </td>
                <td className="px-6 py-3 text-sm text-gray-500 truncate max-w-[200px]">
                  {entry.details ?? ''}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="text-sm text-gray-500 text-center">
        Showing {entries.length} of {response?.total ?? entries.length} entries
      </div>
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
