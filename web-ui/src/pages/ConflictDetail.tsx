import { useState } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api } from '../api';
import DiffViewer from '../components/DiffViewer';

export default function ConflictDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [mergedContent, setMergedContent] = useState('');
  const [activeTab, setActiveTab] = useState<'diff' | 'edit'>('diff');

  const { data: conflict, isLoading } = useQuery({
    queryKey: ['conflict', id],
    queryFn: () => api.getConflict(id!),
    enabled: !!id,
  });

  const resolveMutation = useMutation({
    mutationFn: ({
      resolution,
      content,
    }: {
      resolution: string;
      content?: string;
    }) => api.resolveConflict(id!, resolution, content),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['conflicts'] });
      queryClient.invalidateQueries({ queryKey: ['conflict', id] });
      navigate('/conflicts');
    },
  });

  const deferMutation = useMutation({
    mutationFn: () => api.deferConflict(id!),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['conflicts'] });
      navigate('/conflicts');
    },
  });

  if (isLoading || !conflict) {
    return <div className="text-center py-8 text-gray-500">Loading...</div>;
  }

  const isResolved = conflict.status === 'resolved';

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <button
            onClick={() => navigate('/conflicts')}
            className="text-blue-600 hover:text-blue-900 text-sm mb-2"
          >
            &larr; Back to Conflicts
          </button>
          <h1 className="text-2xl font-bold text-gray-900">
            <code>{conflict.file_path}</code>
          </h1>
        </div>
        <span
          className={`inline-flex items-center px-3 py-1 rounded-full text-sm font-medium ${
            isResolved
              ? 'bg-green-100 text-green-800'
              : 'bg-red-100 text-red-800'
          }`}
        >
          {conflict.status}
        </span>
      </div>

      {/* Metadata */}
      <div className="bg-white shadow rounded-lg p-4 grid grid-cols-4 gap-4 text-sm">
        <div>
          <span className="text-gray-500">Type</span>
          <p className="font-medium capitalize">{conflict.conflict_type}</p>
        </div>
        <div>
          <span className="text-gray-500">SVN Revision</span>
          <p className="font-mono">{conflict.svn_rev ?? '-'}</p>
        </div>
        <div>
          <span className="text-gray-500">Git SHA</span>
          <p className="font-mono truncate">{conflict.git_sha ?? '-'}</p>
        </div>
        <div>
          <span className="text-gray-500">Created</span>
          <p>{new Date(conflict.created_at).toLocaleString()}</p>
        </div>
      </div>

      {/* Tabs */}
      {!isResolved && (
        <div className="border-b border-gray-200">
          <nav className="-mb-px flex space-x-8">
            <button
              onClick={() => setActiveTab('diff')}
              className={`py-4 px-1 border-b-2 font-medium text-sm ${
                activeTab === 'diff'
                  ? 'border-blue-500 text-blue-600'
                  : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300'
              }`}
            >
              Side-by-Side Diff
            </button>
            <button
              onClick={() => {
                setActiveTab('edit');
                if (!mergedContent) {
                  setMergedContent(conflict.git_content ?? conflict.svn_content ?? '');
                }
              }}
              className={`py-4 px-1 border-b-2 font-medium text-sm ${
                activeTab === 'edit'
                  ? 'border-blue-500 text-blue-600'
                  : 'border-transparent text-gray-500 hover:text-gray-700 hover:border-gray-300'
              }`}
            >
              Manual Edit
            </button>
          </nav>
        </div>
      )}

      {/* Content */}
      {activeTab === 'diff' ? (
        <div className="bg-white shadow rounded-lg overflow-hidden">
          <DiffViewer
            oldValue={conflict.svn_content ?? ''}
            newValue={conflict.git_content ?? ''}
            leftTitle="SVN Version"
            rightTitle="Git Version"
          />
        </div>
      ) : (
        <div className="bg-white shadow rounded-lg p-4">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            Edit merged content:
          </label>
          <textarea
            value={mergedContent}
            onChange={(e) => setMergedContent(e.target.value)}
            className="w-full h-96 font-mono text-sm border border-gray-300 rounded-md p-3 focus:ring-blue-500 focus:border-blue-500"
          />
        </div>
      )}

      {/* Resolution Actions */}
      {!isResolved && (
        <div className="bg-white shadow rounded-lg p-4 flex items-center justify-between">
          <div className="flex space-x-3">
            <button
              onClick={() => resolveMutation.mutate({ resolution: 'accept_svn' })}
              disabled={resolveMutation.isPending}
              className="px-4 py-2 bg-purple-600 text-white rounded-md hover:bg-purple-700 disabled:opacity-50 text-sm font-medium"
            >
              Accept SVN
            </button>
            <button
              onClick={() => resolveMutation.mutate({ resolution: 'accept_git' })}
              disabled={resolveMutation.isPending}
              className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 text-sm font-medium"
            >
              Accept Git
            </button>
            {activeTab === 'edit' && (
              <button
                onClick={() =>
                  resolveMutation.mutate({
                    resolution: 'custom',
                    content: mergedContent,
                  })
                }
                disabled={resolveMutation.isPending}
                className="px-4 py-2 bg-green-600 text-white rounded-md hover:bg-green-700 disabled:opacity-50 text-sm font-medium"
              >
                Apply Manual Edit
              </button>
            )}
          </div>
          <button
            onClick={() => deferMutation.mutate()}
            disabled={deferMutation.isPending}
            className="px-4 py-2 bg-gray-200 text-gray-700 rounded-md hover:bg-gray-300 disabled:opacity-50 text-sm"
          >
            Defer
          </button>
        </div>
      )}

      {/* Resolution info */}
      {isResolved && (
        <div className="bg-green-50 border border-green-200 rounded-lg p-4">
          <p className="text-green-800">
            Resolved as <strong>{conflict.resolution}</strong>
            {conflict.resolved_by && <> by {conflict.resolved_by}</>}
            {conflict.resolved_at && (
              <> on {new Date(conflict.resolved_at).toLocaleString()}</>
            )}
          </p>
        </div>
      )}
    </div>
  );
}
