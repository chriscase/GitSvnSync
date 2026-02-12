import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api, type AuthorMapping } from '../api';

export default function Config() {
  const queryClient = useQueryClient();
  const { data: mappings, isLoading } = useQuery({
    queryKey: ['identityMappings'],
    queryFn: api.getIdentityMappings,
  });

  const [newMapping, setNewMapping] = useState<AuthorMapping>({
    svn_username: '',
    name: '',
    email: '',
  });

  const updateMutation = useMutation({
    mutationFn: (updated: AuthorMapping[]) =>
      api.updateIdentityMappings(updated),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ['identityMappings'] }),
  });

  const addMapping = () => {
    if (!newMapping.svn_username || !newMapping.name || !newMapping.email) return;
    const updated = [...(mappings ?? []), newMapping];
    updateMutation.mutate(updated);
    setNewMapping({ svn_username: '', name: '', email: '' });
  };

  const removeMapping = (svnUsername: string) => {
    const updated = (mappings ?? []).filter(
      (m) => m.svn_username !== svnUsername
    );
    updateMutation.mutate(updated);
  };

  if (isLoading) {
    return <div className="text-center py-8 text-gray-500">Loading...</div>;
  }

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-gray-900">Configuration</h1>

      {/* Identity Mappings */}
      <div className="bg-white shadow rounded-lg p-6">
        <h2 className="text-lg font-semibold text-gray-900 mb-4">
          Author Identity Mappings
        </h2>
        <p className="text-sm text-gray-500 mb-4">
          Map SVN usernames to Git author identities. These mappings ensure
          commits are attributed to the correct developer on both sides.
        </p>

        {/* Existing Mappings */}
        <table className="min-w-full divide-y divide-gray-200 mb-4">
          <thead className="bg-gray-50">
            <tr>
              <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase">
                SVN Username
              </th>
              <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase">
                Git Name
              </th>
              <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase">
                Git Email
              </th>
              <th className="px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase">
                GitHub
              </th>
              <th className="px-4 py-2 text-right text-xs font-medium text-gray-500 uppercase">
                Action
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-200">
            {(mappings ?? []).map((m: AuthorMapping) => (
              <tr key={m.svn_username}>
                <td className="px-4 py-2 font-mono text-sm">
                  {m.svn_username}
                </td>
                <td className="px-4 py-2 text-sm">{m.name}</td>
                <td className="px-4 py-2 text-sm">{m.email}</td>
                <td className="px-4 py-2 text-sm text-gray-500">
                  {m.github ?? '-'}
                </td>
                <td className="px-4 py-2 text-right">
                  <button
                    onClick={() => removeMapping(m.svn_username)}
                    className="text-red-600 hover:text-red-900 text-sm"
                  >
                    Remove
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>

        {/* Add New Mapping */}
        <div className="border-t pt-4">
          <h3 className="text-sm font-medium text-gray-700 mb-2">
            Add New Mapping
          </h3>
          <div className="flex space-x-2">
            <input
              placeholder="SVN username"
              value={newMapping.svn_username}
              onChange={(e) =>
                setNewMapping({ ...newMapping, svn_username: e.target.value })
              }
              className="flex-1 rounded-md border border-gray-300 px-3 py-2 text-sm focus:ring-blue-500 focus:border-blue-500"
            />
            <input
              placeholder="Full Name"
              value={newMapping.name}
              onChange={(e) =>
                setNewMapping({ ...newMapping, name: e.target.value })
              }
              className="flex-1 rounded-md border border-gray-300 px-3 py-2 text-sm focus:ring-blue-500 focus:border-blue-500"
            />
            <input
              placeholder="email@company.com"
              value={newMapping.email}
              onChange={(e) =>
                setNewMapping({ ...newMapping, email: e.target.value })
              }
              className="flex-1 rounded-md border border-gray-300 px-3 py-2 text-sm focus:ring-blue-500 focus:border-blue-500"
            />
            <button
              onClick={addMapping}
              disabled={updateMutation.isPending}
              className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 text-sm font-medium"
            >
              Add
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
