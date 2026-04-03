import { LoadingSpinner } from '../components/LoadingSpinner';
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Link } from 'react-router-dom';
import { api, type AuthorMapping } from '../api';

// ---------------------------------------------------------------------------
// Main Config Page
// ---------------------------------------------------------------------------

export default function Config() {
  const queryClient = useQueryClient();
  const { data: config, isLoading: configLoading } = useQuery({
    queryKey: ['config'],
    queryFn: api.getConfig,
  });

  const { data: mappings, isLoading: mappingsLoading } = useQuery({
    queryKey: ['identityMappings'],
    queryFn: api.getIdentityMappings,
  });

  const seedMutation = useMutation({
    mutationFn: api.seedData,
    onSuccess: () => queryClient.invalidateQueries(),
  });

  const [toml, setToml] = useState('');
  const [copied, setCopied] = useState(false);

  if (configLoading || mappingsLoading) {
    return <LoadingSpinner />;
  }

  const generateFullToml = () => {
    if (!config) return;
    const lines: string[] = [];
    lines.push('# RepoSync Configuration');
    lines.push('# Exported from dashboard');
    lines.push('');
    lines.push('[daemon]');
    lines.push(`poll_interval_secs = ${config.daemon.poll_interval_secs}`);
    lines.push(`log_level = "${config.daemon.log_level}"`);
    lines.push(`data_dir = "${config.daemon.data_dir}"`);
    lines.push('');
    lines.push('[svn]');
    lines.push(`url = "${config.svn.url}"`);
    lines.push(`username = "${config.svn.username}"`);
    lines.push(`password_env = "SVN_PASSWORD"`);
    lines.push(`trunk_path = "${config.svn.trunk_path}"`);
    lines.push('');
    lines.push('[github]');
    lines.push(`api_url = "${config.github.api_url}"`);
    lines.push(`repo = "${config.github.repo}"`);
    lines.push(`token_env = "GITEA_TOKEN"`);
    lines.push(`default_branch = "${config.github.default_branch}"`);
    lines.push('');
    lines.push('[web]');
    lines.push(`listen = "${config.web.listen}"`);
    lines.push(`auth_mode = "${config.web.auth_mode.toLowerCase()}"`);
    lines.push('admin_password_env = "ADMIN_PASSWORD"');
    lines.push('');
    lines.push('[sync]');
    lines.push(`mode = "${config.sync.mode.toLowerCase()}"`);
    lines.push(`auto_merge = ${config.sync.auto_merge}`);
    lines.push(`sync_tags = ${config.sync.sync_tags}`);
    lines.push('');
    setToml(lines.join('\n'));
  };

  const handleCopy = async () => {
    await navigator.clipboard.writeText(toml);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleDownload = () => {
    const blob = new Blob([toml], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'gitsvnsync.toml';
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-gray-100">Configuration</h1>
          <p className="text-sm text-gray-400 mt-1">View and manage your RepoSync settings</p>
        </div>
        <div className="flex items-center space-x-3">
          <Link
            to="/setup"
            className="px-4 py-2 border border-blue-600 text-blue-400 rounded-lg hover:bg-blue-600/10 text-sm font-medium transition-colors"
          >
            Setup Wizard
          </Link>
          <button
            onClick={() => seedMutation.mutate()}
            disabled={seedMutation.isPending}
            className="px-4 py-2 bg-emerald-600 text-white rounded-lg hover:bg-emerald-700 disabled:opacity-50 text-sm font-medium transition-colors"
          >
            {seedMutation.isPending ? 'Seeding...' : 'Seed Demo Data'}
          </button>
        </div>
      </div>

      {seedMutation.isSuccess && (
        <div className="bg-green-900/30 border border-green-700 rounded-lg p-3">
          <p className="text-green-300 text-sm">Demo data seeded successfully! Refresh pages to see new data.</p>
        </div>
      )}

      {/* Config sections */}
      {config && (
        <div className="space-y-6">
          <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-6">
            <ConfigSection title="Daemon Settings" color="emerald" items={[
              { label: 'Poll Interval', value: `${config.daemon.poll_interval_secs}s` },
              { label: 'Log Level', value: config.daemon.log_level },
              { label: 'Data Directory', value: config.daemon.data_dir, mono: true },
            ]} />
            <ConfigSection title="Sync Configuration" color="blue" items={[
              { label: 'Mode', value: config.sync.mode },
              { label: 'Auto Merge', value: config.sync.auto_merge ? 'Enabled' : 'Disabled', color: config.sync.auto_merge ? 'text-green-400' : 'text-red-400' },
              { label: 'Sync Tags', value: config.sync.sync_tags ? 'Enabled' : 'Disabled', color: config.sync.sync_tags ? 'text-green-400' : 'text-red-400' },
            ]} />
            <ConfigSection title="SVN Repository" color="orange" items={[
              { label: 'URL', value: config.svn.url, mono: true },
              { label: 'Username', value: config.svn.username },
              { label: 'Password', value: config.svn.password },
              { label: 'Trunk Path', value: config.svn.trunk_path, mono: true },
            ]} />
            <ConfigSection title="Git Repository" color="purple" items={[
              { label: 'API URL', value: config.github.api_url, mono: true },
              { label: 'Repository', value: config.github.repo },
              { label: 'Token', value: config.github.token },
              { label: 'Default Branch', value: config.github.default_branch },
            ]} />
            <ConfigSection title="Web Server" color="cyan" items={[
              { label: 'Listen Address', value: config.web.listen, mono: true },
              { label: 'Auth Mode', value: config.web.auth_mode },
            ]} />
          </div>

          {/* Export TOML */}
          <div className="bg-gray-800 rounded-lg p-6 border border-gray-700">
            <div className="flex items-center justify-between mb-4">
              <div>
                <h2 className="text-lg font-semibold text-gray-100">Export Configuration</h2>
                <p className="text-sm text-gray-400 mt-1">Generate a TOML config file from current settings</p>
              </div>
              <div className="flex items-center space-x-2">
                {!toml ? (
                  <button onClick={generateFullToml} className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 text-sm font-medium transition-colors">
                    Generate TOML
                  </button>
                ) : (
                  <>
                    <button onClick={generateFullToml} className="px-3 py-1.5 text-xs text-gray-400 hover:text-gray-200 border border-gray-600 rounded-lg hover:bg-gray-700 transition-colors">
                      Regenerate
                    </button>
                    <button
                      onClick={handleCopy}
                      className={`px-3 py-1.5 text-xs rounded-lg border transition-colors ${copied ? 'border-emerald-600 text-emerald-400 bg-emerald-600/10' : 'border-gray-600 text-gray-300 hover:bg-gray-700'}`}
                    >
                      {copied ? 'Copied!' : 'Copy'}
                    </button>
                    <button onClick={handleDownload} className="px-3 py-1.5 text-xs bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors">
                      Download
                    </button>
                  </>
                )}
              </div>
            </div>
            {toml && (
              <pre className="bg-gray-900 border border-gray-700 rounded-lg p-5 text-sm font-mono text-gray-300 overflow-x-auto max-h-[400px] overflow-y-auto leading-relaxed whitespace-pre">
                {toml}
              </pre>
            )}
          </div>
        </div>
      )}

      {/* Identity Mappings */}
      <IdentityMappingsEditor mappings={mappings ?? []} />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface ConfigItem {
  label: string;
  value: string;
  mono?: boolean;
  color?: string;
}

function ConfigSection({ title, color, items }: { title: string; color: string; items: ConfigItem[] }) {
  const dotColors: Record<string, string> = {
    emerald: 'bg-emerald-400', blue: 'bg-blue-400', orange: 'bg-orange-400',
    purple: 'bg-purple-400', cyan: 'bg-cyan-400',
  };

  return (
    <div className="bg-gray-800 shadow rounded-lg p-6 border border-gray-700">
      <h2 className="text-lg font-semibold text-gray-100 mb-4 flex items-center space-x-2">
        <span className={`w-2 h-2 rounded-full ${dotColors[color] ?? 'bg-gray-400'}`} />
        <span>{title}</span>
      </h2>
      <dl className="space-y-3">
        {items.map(item => (
          <div key={item.label} className="flex items-center justify-between">
            <dt className="text-sm text-gray-400">{item.label}</dt>
            <dd className={`text-sm ${item.color ?? 'text-gray-200'} ${item.mono ? 'font-mono' : ''}`}>
              {item.value}
            </dd>
          </div>
        ))}
      </dl>
    </div>
  );
}

function IdentityMappingsEditor({ mappings: initialMappings }: { mappings: AuthorMapping[] }) {
  const queryClient = useQueryClient();
  const [newMapping, setNewMapping] = useState<AuthorMapping>({ svn_username: '', name: '', email: '' });

  const updateMutation = useMutation({
    mutationFn: (updated: AuthorMapping[]) => api.updateIdentityMappings(updated),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['identityMappings'] }),
  });

  const addMapping = () => {
    if (!newMapping.svn_username || !newMapping.name || !newMapping.email) return;
    const updated = [...initialMappings, newMapping];
    updateMutation.mutate(updated);
    setNewMapping({ svn_username: '', name: '', email: '' });
  };

  const removeMapping = (svnUsername: string) => {
    const updated = initialMappings.filter(m => m.svn_username !== svnUsername);
    updateMutation.mutate(updated);
  };

  return (
    <div className="bg-gray-800 shadow rounded-lg p-6 border border-gray-700">
      <h2 className="text-lg font-semibold text-gray-100 mb-2 flex items-center space-x-2">
        <span className="w-2 h-2 rounded-full bg-green-400" />
        <span>Author Identity Mappings</span>
      </h2>
      <p className="text-sm text-gray-400 mb-4 ml-4">
        Map SVN usernames to Git author identities. These mappings ensure commits are attributed to the correct developer.
      </p>

      {initialMappings.length > 0 ? (
        <div className="overflow-x-auto mb-4">
          <table className="min-w-full divide-y divide-gray-700">
            <thead>
              <tr>
                <th className="px-4 py-2 text-left text-xs font-medium text-gray-400 uppercase">SVN Username</th>
                <th className="px-4 py-2 text-left text-xs font-medium text-gray-400 uppercase">Git Name</th>
                <th className="px-4 py-2 text-left text-xs font-medium text-gray-400 uppercase">Git Email</th>
                <th className="px-4 py-2 text-left text-xs font-medium text-gray-400 uppercase">GitHub</th>
                <th className="px-4 py-2 text-right text-xs font-medium text-gray-400 uppercase">Action</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-gray-700">
              {initialMappings.map(m => (
                <tr key={m.svn_username} className="hover:bg-gray-700/50">
                  <td className="px-4 py-2 font-mono text-sm text-gray-200">{m.svn_username}</td>
                  <td className="px-4 py-2 text-sm text-gray-300">{m.name}</td>
                  <td className="px-4 py-2 text-sm text-gray-300">{m.email}</td>
                  <td className="px-4 py-2 text-sm text-gray-400">{m.github ?? '-'}</td>
                  <td className="px-4 py-2 text-right">
                    <button onClick={() => removeMapping(m.svn_username)} className="text-red-400 hover:text-red-300 text-sm">
                      Remove
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="text-center py-8 text-gray-500 mb-4">No identity mappings configured yet.</div>
      )}

      <div className="border-t border-gray-700 pt-4">
        <h3 className="text-sm font-medium text-gray-300 mb-2">Add New Mapping</h3>
        <div className="flex space-x-2">
          <input
            placeholder="SVN username"
            value={newMapping.svn_username}
            onChange={e => setNewMapping({ ...newMapping, svn_username: e.target.value })}
            className="flex-1 rounded-lg border border-gray-600 bg-gray-700 text-gray-100 placeholder-gray-400 px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500 focus:outline-none font-mono"
          />
          <input
            placeholder="Full Name"
            value={newMapping.name}
            onChange={e => setNewMapping({ ...newMapping, name: e.target.value })}
            className="flex-1 rounded-lg border border-gray-600 bg-gray-700 text-gray-100 placeholder-gray-400 px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500 focus:outline-none"
          />
          <input
            placeholder="email@company.com"
            value={newMapping.email}
            onChange={e => setNewMapping({ ...newMapping, email: e.target.value })}
            className="flex-1 rounded-lg border border-gray-600 bg-gray-700 text-gray-100 placeholder-gray-400 px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500 focus:outline-none"
          />
          <button
            onClick={addMapping}
            disabled={updateMutation.isPending}
            className="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 text-sm font-medium transition-colors"
          >
            Add
          </button>
        </div>
      </div>
    </div>
  );
}
