import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { api, type AuditEntry } from '../api';

export default function AuditLog() {
  const [offset, setOffset] = useState(0);
  const limit = 50;

  const { data: entries, isLoading } = useQuery({
    queryKey: ['audit', offset],
    queryFn: () => api.getAuditLog(limit, offset),
  });

  if (isLoading) {
    return <div className="text-center py-8 text-gray-500">Loading...</div>;
  }

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
            {(entries ?? []).map((entry: AuditEntry) => (
              <tr key={entry.id} className="hover:bg-gray-50">
                <td className="px-6 py-3 text-sm text-gray-500 whitespace-nowrap">
                  {new Date(entry.created_at).toLocaleString()}
                </td>
                <td className="px-6 py-3 whitespace-nowrap">
                  <DirectionBadge direction={entry.direction} />
                </td>
                <td className="px-6 py-3 text-sm text-gray-900">
                  {entry.action}
                </td>
                <td className="px-6 py-3 text-sm text-gray-700">
                  {entry.author}
                </td>
                <td className="px-6 py-3 text-sm font-mono text-gray-500">
                  {entry.svn_rev ?? '-'}
                </td>
                <td className="px-6 py-3 text-sm font-mono text-gray-500 truncate max-w-[120px]">
                  {entry.git_sha ? entry.git_sha.substring(0, 8) : '-'}
                </td>
                <td className="px-6 py-3 text-sm text-gray-500 truncate max-w-[200px]">
                  {entry.details}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      <div className="flex justify-between items-center">
        <button
          onClick={() => setOffset(Math.max(0, offset - limit))}
          disabled={offset === 0}
          className="px-4 py-2 bg-gray-200 text-gray-700 rounded-md hover:bg-gray-300 disabled:opacity-50 text-sm"
        >
          Previous
        </button>
        <span className="text-sm text-gray-500">
          Showing {offset + 1} - {offset + (entries?.length ?? 0)}
        </span>
        <button
          onClick={() => setOffset(offset + limit)}
          disabled={(entries?.length ?? 0) < limit}
          className="px-4 py-2 bg-gray-200 text-gray-700 rounded-md hover:bg-gray-300 disabled:opacity-50 text-sm"
        >
          Next
        </button>
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
