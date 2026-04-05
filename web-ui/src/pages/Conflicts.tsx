import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { api, type Conflict } from '../api';

export default function Conflicts() {
  const [filter, setFilter] = useState<string>('');
  const { data: conflicts, isLoading } = useQuery({
    queryKey: ['conflicts', filter],
    queryFn: () => api.getConflicts(filter || undefined),
  });

  if (isLoading) {
    return <div className="text-center py-8 text-gray-500">Loading...</div>;
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-900">Conflicts</h1>
        <div className="flex space-x-2">
          {['', 'detected', 'queued', 'deferred', 'resolved'].map((f) => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={`px-3 py-1 rounded-md text-sm ${
                filter === f
                  ? 'bg-gray-900 text-white'
                  : 'bg-gray-200 text-gray-700 hover:bg-gray-300'
              }`}
            >
              {f === '' ? 'All' : f.charAt(0).toUpperCase() + f.slice(1)}
            </button>
          ))}
        </div>
      </div>

      {conflicts && conflicts.length > 0 ? (
        <div className="bg-white shadow overflow-hidden rounded-lg">
          <table className="min-w-full divide-y divide-gray-200">
            <thead className="bg-gray-50">
              <tr>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                  File
                </th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                  Type
                </th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                  Status
                </th>
                <th className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                  Created
                </th>
                <th className="px-6 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wider">
                  Action
                </th>
              </tr>
            </thead>
            <tbody className="bg-white divide-y divide-gray-200">
              {conflicts.map((conflict: Conflict) => (
                <ConflictRow key={conflict.id} conflict={conflict} />
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="bg-white shadow rounded-lg p-12 text-center">
          <p className="text-gray-500 text-lg">No conflicts found</p>
          <p className="text-gray-400 text-sm mt-1">
            Everything is in sync!
          </p>
        </div>
      )}
    </div>
  );
}

function ConflictRow({ conflict }: { conflict: Conflict }) {
  const statusColors: Record<string, string> = {
    detected: 'bg-red-100 text-red-800',
    queued: 'bg-yellow-100 text-yellow-800',
    deferred: 'bg-gray-100 text-gray-800',
    resolved: 'bg-green-100 text-green-800',
  };

  const typeLabels: Record<string, string> = {
    content: 'Content',
    edit_delete: 'Edit/Delete',
    rename: 'Rename',
    property: 'Property',
    branch: 'Branch',
    binary: 'Binary',
  };

  return (
    <tr className="hover:bg-gray-50">
      <td className="px-6 py-4 whitespace-nowrap">
        <code className="text-sm font-mono text-gray-900">
          {conflict.file_path}
        </code>
      </td>
      <td className="px-6 py-4 whitespace-nowrap">
        <span className="text-sm text-gray-700">
          {typeLabels[conflict.conflict_type] ?? conflict.conflict_type}
        </span>
      </td>
      <td className="px-6 py-4 whitespace-nowrap">
        <span
          className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium ${
            statusColors[conflict.status] ?? 'bg-gray-100 text-gray-800'
          }`}
        >
          {conflict.status}
        </span>
      </td>
      <td className="px-6 py-4 whitespace-nowrap text-sm text-gray-500">
        {new Date(conflict.created_at).toLocaleString()}
      </td>
      <td className="px-6 py-4 whitespace-nowrap text-right">
        <Link
          to={`/conflicts/${conflict.id}`}
          className="text-blue-600 hover:text-blue-900 text-sm font-medium"
        >
          {conflict.status === 'resolved' ? 'View' : 'Resolve'}
        </Link>
      </td>
    </tr>
  );
}
