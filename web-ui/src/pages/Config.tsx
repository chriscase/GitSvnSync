import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { api, type AuthorMapping, type ConfigResponse } from '../api';

export default function Config() {
  const queryClient = useQueryClient();
  const { data: mappings, isLoading: mappingsLoading } = useQuery({
    queryKey: ['identityMappings'],
    queryFn: api.getIdentityMappings,
  });

  const { data: config, isLoading: configLoading } = useQuery({
    queryKey: ['config'],
    queryFn: api.getConfig,
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

  const seedMutation = useMutation({
    mutationFn: api.seedData,
    onSuccess: () => {
      queryClient.invalidateQueries();
    },
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

  if (mappingsLoading || configLoading) {
    return <div className="text-center py-8 text-gray-400">Loading...</div>;
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-100">Configuration</h1>
        <button
          onClick={() => seedMutation.mutate()}
          disabled={seedMutation.isPending}
          className="px-4 py-2 bg-emerald-600 text-white rounded-md hover:bg-emerald-700 disabled:opacity-50 text-sm font-medium"
        >
          {seedMutation.isPending ? 'Seeding...' : 'Seed Demo Data'}
        </button>
      </div>

      {seedMutation.isSuccess && (
        <div className="bg-green-900/30 border border-green-700 rounded-lg p-3">
          <p className="text-green-300 text-sm">Demo data seeded successfully! Refresh pages to see new data.</p>
        </div>
      )}

      {/* Server Configuration */}
      {config && <ServerConfig config={config} />}

      {/* Identity Mappings */}
      <div className="bg-gray-800 shadow rounded-lg p-6 border border-gray-700">
        <h2 className="text-lg font-semibold text-gray-100 mb-2">
          Author Identity Mappings
        </h2>
        <p className="text-sm text-gray-400 mb-4">
          Map SVN usernames to Git author identities. These mappings ensure
          commits are attributed to the correct developer on both sides.
        </p>

        {/* Existing Mappings */}
        {(mappings ?? []).length > 0 ? (
          <div className="overflow-x-auto">
            <table className="min-w-full divide-y divide-gray-700 mb-4">
              <thead>
                <tr>
                  <th className="px-4 py-2 text-left text-xs font-medium text-gray-400 uppercase">
                    SVN Username
                  </th>
                  <th className="px-4 py-2 text-left text-xs font-medium text-gray-400 uppercase">
                    Git Name
                  </th>
                  <th className="px-4 py-2 text-left text-xs font-medium text-gray-400 uppercase">
                    Git Email
                  </th>
                  <th className="px-4 py-2 text-left text-xs font-medium text-gray-400 uppercase">
                    GitHub
                  </th>
                  <th className="px-4 py-2 text-right text-xs font-medium text-gray-400 uppercase">
                    Action
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-700">
                {(mappings ?? []).map((m: AuthorMapping) => (
                  <tr key={m.svn_username} className="hover:bg-gray-700/50">
                    <td className="px-4 py-2 font-mono text-sm text-gray-200">
                      {m.svn_username}
                    </td>
                    <td className="px-4 py-2 text-sm text-gray-300">{m.name}</td>
                    <td className="px-4 py-2 text-sm text-gray-300">{m.email}</td>
                    <td className="px-4 py-2 text-sm text-gray-400">
                      {m.github ?? '-'}
                    </td>
                    <td className="px-4 py-2 text-right">
                      <button
                        onClick={() => removeMapping(m.svn_username)}
                        className="text-red-400 hover:text-red-300 text-sm"
                      >
                        Remove
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="text-center py-8 text-gray-500 mb-4">
            No identity mappings configured yet.
          </div>
        )}

        {/* Add New Mapping */}
        <div className="border-t border-gray-700 pt-4">
          <h3 className="text-sm font-medium text-gray-300 mb-2">
            Add New Mapping
          </h3>
          <div className="flex space-x-2">
            <input
              placeholder="SVN username"
              value={newMapping.svn_username}
              onChange={(e) =>
                setNewMapping({ ...newMapping, svn_username: e.target.value })
              }
              className="flex-1 rounded-md border border-gray-600 bg-gray-700 text-gray-100 placeholder-gray-400 px-3 py-2 text-sm focus:ring-blue-500 focus:border-blue-500"
            />
            <input
              placeholder="Full Name"
              value={newMapping.name}
              onChange={(e) =>
                setNewMapping({ ...newMapping, name: e.target.value })
              }
              className="flex-1 rounded-md border border-gray-600 bg-gray-700 text-gray-100 placeholder-gray-400 px-3 py-2 text-sm focus:ring-blue-500 focus:border-blue-500"
            />
            <input
              placeholder="email@company.com"
              value={newMapping.email}
              onChange={(e) =>
                setNewMapping({ ...newMapping, email: e.target.value })
              }
              className="flex-1 rounded-md border border-gray-600 bg-gray-700 text-gray-100 placeholder-gray-400 px-3 py-2 text-sm focus:ring-blue-500 focus:border-blue-500"
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

function ServerConfig({ config }: { config: ConfigResponse }) {
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
      {/* Daemon Settings */}
      <div className="bg-gray-800 shadow rounded-lg p-6 border border-gray-700">
        <h2 className="text-lg font-semibold text-gray-100 mb-4 flex items-center space-x-2">
          <span className="w-2 h-2 rounded-full bg-emerald-400 inline-block" />
          <span>Daemon Settings</span>
        </h2>
        <dl className="space-y-3">
          <ConfigRow label="Poll Interval" value={`${config.daemon.poll_interval_secs}s`} />
          <ConfigRow label="Log Level" value={config.daemon.log_level} />
          <ConfigRow label="Data Directory" value={config.daemon.data_dir} mono />
        </dl>
      </div>

      {/* Sync Settings */}
      <div className="bg-gray-800 shadow rounded-lg p-6 border border-gray-700">
        <h2 className="text-lg font-semibold text-gray-100 mb-4 flex items-center space-x-2">
          <span className="w-2 h-2 rounded-full bg-blue-400 inline-block" />
          <span>Sync Configuration</span>
        </h2>
        <dl className="space-y-3">
          <ConfigRow label="Mode" value={config.sync.mode} />
          <ConfigRow
            label="Auto Merge"
            value={config.sync.auto_merge ? 'Enabled' : 'Disabled'}
            color={config.sync.auto_merge ? 'text-green-400' : 'text-red-400'}
          />
          <ConfigRow
            label="Sync Tags"
            value={config.sync.sync_tags ? 'Enabled' : 'Disabled'}
            color={config.sync.sync_tags ? 'text-green-400' : 'text-red-400'}
          />
        </dl>
      </div>

      {/* SVN Configuration */}
      <div className="bg-gray-800 shadow rounded-lg p-6 border border-gray-700">
        <h2 className="text-lg font-semibold text-gray-100 mb-4 flex items-center space-x-2">
          <span className="w-2 h-2 rounded-full bg-orange-400 inline-block" />
          <span>SVN Repository</span>
        </h2>
        <dl className="space-y-3">
          <ConfigRow label="URL" value={config.svn.url} mono />
          <ConfigRow label="Username" value={config.svn.username} />
          <ConfigRow label="Password" value={config.svn.password} />
          <ConfigRow label="Trunk Path" value={config.svn.trunk_path} mono />
        </dl>
      </div>

      {/* GitHub/Gitea Configuration */}
      <div className="bg-gray-800 shadow rounded-lg p-6 border border-gray-700">
        <h2 className="text-lg font-semibold text-gray-100 mb-4 flex items-center space-x-2">
          <span className="w-2 h-2 rounded-full bg-purple-400 inline-block" />
          <span>Git Repository</span>
        </h2>
        <dl className="space-y-3">
          <ConfigRow label="API URL" value={config.github.api_url} mono />
          <ConfigRow label="Repository" value={config.github.repo} />
          <ConfigRow label="Token" value={config.github.token} />
          <ConfigRow label="Default Branch" value={config.github.default_branch} />
        </dl>
      </div>

      {/* Web Settings */}
      <div className="bg-gray-800 shadow rounded-lg p-6 border border-gray-700 md:col-span-2">
        <h2 className="text-lg font-semibold text-gray-100 mb-4 flex items-center space-x-2">
          <span className="w-2 h-2 rounded-full bg-cyan-400 inline-block" />
          <span>Web Server</span>
        </h2>
        <dl className="grid grid-cols-2 gap-3">
          <ConfigRow label="Listen Address" value={config.web.listen} mono />
          <ConfigRow label="Auth Mode" value={config.web.auth_mode} />
        </dl>
      </div>
    </div>
  );
}

function ConfigRow({
  label,
  value,
  mono,
  color,
}: {
  label: string;
  value: string;
  mono?: boolean;
  color?: string;
}) {
  return (
    <div className="flex items-center justify-between">
      <dt className="text-sm text-gray-400">{label}</dt>
      <dd
        className={`text-sm ${color ?? 'text-gray-200'} ${
          mono ? 'font-mono' : ''
        }`}
      >
        {value}
      </dd>
    </div>
  );
}
