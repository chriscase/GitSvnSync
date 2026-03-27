import { useState, useRef, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router-dom';
import { api, type AuthorMapping } from '../api';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface WizardData {
  // SVN
  svn_url: string;
  svn_username: string;
  svn_password_env: string;
  svn_layout: 'standard' | 'custom';
  svn_trunk_path: string;
  svn_branches_path: string;
  svn_tags_path: string;

  // Git
  git_provider: 'github' | 'gitea';
  git_api_url: string;
  git_repo: string;
  git_token_env: string;
  git_default_branch: string;

  // Sync
  sync_mode: 'direct' | 'pr';
  sync_auto_merge: boolean;
  sync_tags: boolean;
  pr_title_prefix: string;
  pr_labels: string;
  pr_reviewers: string;
  pr_auto_merge: boolean;

  // Identity
  identity_email_domain: string;
  identity_mapping_file: string;
  identity_mappings: AuthorMapping[];

  // Web & Auth
  web_listen: string;
  web_auth_mode: 'simple' | 'github_oauth' | 'both';
  web_admin_password_env: string;
  web_oauth_client_id: string;
  web_oauth_client_secret_env: string;
  web_oauth_allowed_users: string;

  // Daemon
  daemon_poll_interval: number;
  daemon_log_level: string;
  daemon_data_dir: string;

  // Notifications
  notif_slack_webhook_env: string;
  notif_email_smtp: string;
  notif_email_from: string;
  notif_email_recipients: string;
}

const DEFAULT_DATA: WizardData = {
  svn_url: '',
  svn_username: '',
  svn_password_env: 'GITSVNSYNC_SVN_PASSWORD',
  svn_layout: 'standard',
  svn_trunk_path: 'trunk',
  svn_branches_path: 'branches',
  svn_tags_path: 'tags',

  git_provider: 'github',
  git_api_url: 'https://api.github.com',
  git_repo: '',
  git_token_env: 'GITSVNSYNC_GITHUB_TOKEN',
  git_default_branch: 'main',

  sync_mode: 'direct',
  sync_auto_merge: true,
  sync_tags: true,
  pr_title_prefix: '[svn-sync]',
  pr_labels: '',
  pr_reviewers: '',
  pr_auto_merge: false,

  identity_email_domain: '',
  identity_mapping_file: '',
  identity_mappings: [],

  web_listen: '0.0.0.0:8080',
  web_auth_mode: 'simple',
  web_admin_password_env: 'GITSVNSYNC_ADMIN_PASSWORD',
  web_oauth_client_id: '',
  web_oauth_client_secret_env: '',
  web_oauth_allowed_users: '',

  daemon_poll_interval: 60,
  daemon_log_level: 'info',
  daemon_data_dir: '/var/lib/gitsvnsync',

  notif_slack_webhook_env: '',
  notif_email_smtp: '',
  notif_email_from: '',
  notif_email_recipients: '',
};

const STEPS = [
  { label: 'Welcome', short: 'Start' },
  { label: 'SVN Repository', short: 'SVN' },
  { label: 'Git Provider', short: 'Git' },
  { label: 'Sync Settings', short: 'Sync' },
  { label: 'Identity Mapping', short: 'Identity' },
  { label: 'Server & Auth', short: 'Server' },
  { label: 'Review & Generate', short: 'Review' },
];

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

type Errors = Record<string, string>;

function validateStep(step: number, data: WizardData): Errors {
  const errors: Errors = {};

  if (step === 1) {
    if (!data.svn_url.trim()) errors.svn_url = 'SVN URL is required';
    else if (!/^(svn|https?):\/\/.+/.test(data.svn_url.trim()))
      errors.svn_url = 'Must start with svn://, http://, or https://';
    if (!data.svn_username.trim()) errors.svn_username = 'Username is required';
    if (!data.svn_password_env.trim()) errors.svn_password_env = 'Environment variable name is required';
    else if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(data.svn_password_env.trim()))
      errors.svn_password_env = 'Must be a valid env var name (letters, digits, underscores)';
    if (!data.svn_trunk_path.trim()) errors.svn_trunk_path = 'Trunk path is required';
  }

  if (step === 2) {
    if (!data.git_api_url.trim()) errors.git_api_url = 'API URL is required';
    else if (!/^https?:\/\/.+/.test(data.git_api_url.trim()))
      errors.git_api_url = 'Must be a valid HTTP(S) URL';
    if (!data.git_repo.trim()) errors.git_repo = 'Repository is required';
    else if (!/^[^/]+\/[^/]+$/.test(data.git_repo.trim()))
      errors.git_repo = 'Must be in owner/repo format';
    if (!data.git_token_env.trim()) errors.git_token_env = 'Token env var is required';
    else if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(data.git_token_env.trim()))
      errors.git_token_env = 'Must be a valid env var name';
  }

  if (step === 5) {
    if (!data.web_listen.trim()) errors.web_listen = 'Listen address is required';
    else if (!/^.+:\d+$/.test(data.web_listen.trim()))
      errors.web_listen = 'Must be in host:port format (e.g. 0.0.0.0:8080)';
    if (data.daemon_poll_interval < 1) errors.daemon_poll_interval = 'Must be at least 1 second';
    if (!data.daemon_data_dir.trim()) errors.daemon_data_dir = 'Data directory is required';
    if ((data.web_auth_mode === 'simple' || data.web_auth_mode === 'both') && !data.web_admin_password_env.trim())
      errors.web_admin_password_env = 'Admin password env var is required for simple auth';
    if (data.identity_email_domain && !/^[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$/.test(data.identity_email_domain.trim()))
      errors.identity_email_domain = 'Must be a valid domain (e.g. company.com)';
  }

  return errors;
}

// ---------------------------------------------------------------------------
// TOML Generation
// ---------------------------------------------------------------------------

function tomlStr(val: string): string {
  return `"${val.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}

function generateToml(data: WizardData): string {
  const lines: string[] = [];

  lines.push('# GitSvnSync Configuration');
  lines.push('# Generated by the Setup Wizard');
  lines.push('');

  // Daemon
  lines.push('# ── Daemon ──────────────────────────────────────────────');
  lines.push('[daemon]');
  lines.push(`poll_interval_secs = ${data.daemon_poll_interval}`);
  lines.push(`log_level = ${tomlStr(data.daemon_log_level)}`);
  lines.push(`data_dir = ${tomlStr(data.daemon_data_dir)}`);
  lines.push('');

  // SVN
  lines.push('# ── SVN Repository ─────────────────────────────────────');
  lines.push('[svn]');
  lines.push(`url = ${tomlStr(data.svn_url)}`);
  lines.push(`username = ${tomlStr(data.svn_username)}`);
  lines.push(`password_env = ${tomlStr(data.svn_password_env)}`);
  lines.push(`layout = ${tomlStr(data.svn_layout)}`);
  lines.push(`trunk_path = ${tomlStr(data.svn_trunk_path)}`);
  if (data.svn_layout === 'custom' || data.svn_branches_path !== 'branches') {
    lines.push(`branches_path = ${tomlStr(data.svn_branches_path)}`);
  }
  if (data.svn_layout === 'custom' || data.svn_tags_path !== 'tags') {
    lines.push(`tags_path = ${tomlStr(data.svn_tags_path)}`);
  }
  lines.push('');

  // GitHub / Gitea
  lines.push('# ── Git Provider ───────────────────────────────────────');
  lines.push('[github]');
  lines.push(`api_url = ${tomlStr(data.git_api_url)}`);
  lines.push(`repo = ${tomlStr(data.git_repo)}`);
  lines.push(`token_env = ${tomlStr(data.git_token_env)}`);
  lines.push(`default_branch = ${tomlStr(data.git_default_branch)}`);
  if (data.git_provider !== 'github') {
    lines.push(`provider = ${tomlStr(data.git_provider)}`);
  }
  lines.push('');

  // Identity
  const hasIdentity = data.identity_email_domain || data.identity_mapping_file;
  if (hasIdentity) {
    lines.push('# ── Identity Mapping ───────────────────────────────────');
    lines.push('[identity]');
    if (data.identity_email_domain) lines.push(`email_domain = ${tomlStr(data.identity_email_domain)}`);
    if (data.identity_mapping_file) lines.push(`mapping_file = ${tomlStr(data.identity_mapping_file)}`);
    lines.push('');
  }

  // Web
  lines.push('# ── Web Server ─────────────────────────────────────────');
  lines.push('[web]');
  lines.push(`listen = ${tomlStr(data.web_listen)}`);
  lines.push(`auth_mode = ${tomlStr(data.web_auth_mode)}`);
  if (data.web_auth_mode === 'simple' || data.web_auth_mode === 'both') {
    lines.push(`admin_password_env = ${tomlStr(data.web_admin_password_env)}`);
  }
  if (data.web_auth_mode === 'github_oauth' || data.web_auth_mode === 'both') {
    if (data.web_oauth_client_id) lines.push(`oauth_client_id = ${tomlStr(data.web_oauth_client_id)}`);
    if (data.web_oauth_client_secret_env) lines.push(`oauth_client_secret_env = ${tomlStr(data.web_oauth_client_secret_env)}`);
    if (data.web_oauth_allowed_users) {
      const users = data.web_oauth_allowed_users.split(',').map(u => tomlStr(u.trim())).join(', ');
      lines.push(`oauth_allowed_users = [${users}]`);
    }
  }
  lines.push('');

  // Notifications
  const hasNotif = data.notif_slack_webhook_env || data.notif_email_smtp;
  if (hasNotif) {
    lines.push('# ── Notifications ──────────────────────────────────────');
    lines.push('[notifications]');
    if (data.notif_slack_webhook_env) lines.push(`slack_webhook_url_env = ${tomlStr(data.notif_slack_webhook_env)}`);
    if (data.notif_email_smtp) lines.push(`email_smtp = ${tomlStr(data.notif_email_smtp)}`);
    if (data.notif_email_from) lines.push(`email_from = ${tomlStr(data.notif_email_from)}`);
    if (data.notif_email_recipients) {
      const recips = data.notif_email_recipients.split(',').map(r => tomlStr(r.trim())).join(', ');
      lines.push(`email_recipients = [${recips}]`);
    }
    lines.push('');
  }

  // Sync
  lines.push('# ── Sync Behavior ──────────────────────────────────────');
  lines.push('[sync]');
  lines.push(`mode = ${tomlStr(data.sync_mode)}`);
  lines.push(`auto_merge = ${data.sync_auto_merge}`);
  lines.push(`sync_tags = ${data.sync_tags}`);
  if (data.sync_mode === 'pr') {
    lines.push('');
    lines.push('[sync.pr]');
    lines.push(`title_prefix = ${tomlStr(data.pr_title_prefix)}`);
    lines.push(`auto_merge = ${data.pr_auto_merge}`);
    if (data.pr_labels) {
      const labels = data.pr_labels.split(',').map(l => tomlStr(l.trim())).join(', ');
      lines.push(`labels = [${labels}]`);
    }
    if (data.pr_reviewers) {
      const reviewers = data.pr_reviewers.split(',').map(r => tomlStr(r.trim())).join(', ');
      lines.push(`reviewers = [${reviewers}]`);
    }
  }
  lines.push('');

  return lines.join('\n');
}

// ---------------------------------------------------------------------------
// Main Component
// ---------------------------------------------------------------------------

export default function SetupWizard() {
  const navigate = useNavigate();
  const [step, setStep] = useState(0);
  const [data, setData] = useState<WizardData>({ ...DEFAULT_DATA });
  const [errors, setErrors] = useState<Errors>({});
  const [toml, setToml] = useState('');
  const [copied, setCopied] = useState(false);
  const stepRef = useRef<HTMLDivElement>(null);

  const update = useCallback(
    (fields: Partial<WizardData>) => setData(prev => ({ ...prev, ...fields })),
    [],
  );

  useEffect(() => {
    stepRef.current?.querySelector<HTMLInputElement>('input, select, textarea')?.focus();
  }, [step]);

  const goNext = () => {
    const errs = validateStep(step, data);
    setErrors(errs);
    if (Object.keys(errs).length > 0) return;
    if (step === STEPS.length - 2) {
      setToml(generateToml(data));
    }
    setStep(s => Math.min(s + 1, STEPS.length - 1));
  };

  const goBack = () => {
    setErrors({});
    setStep(s => Math.max(s - 1, 0));
  };

  const copyToClipboard = async () => {
    await navigator.clipboard.writeText(toml);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const downloadToml = () => {
    const blob = new Blob([toml], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'gitsvnsync.toml';
    a.click();
    URL.revokeObjectURL(url);
  };

  const renderStep = () => {
    switch (step) {
      case 0: return <WelcomeStep />;
      case 1: return <SvnStep data={data} update={update} errors={errors} />;
      case 2: return <GitStep data={data} update={update} errors={errors} />;
      case 3: return <SyncStep data={data} update={update} />;
      case 4: return <IdentityStep data={data} update={update} errors={errors} />;
      case 5: return <ServerAuthStep data={data} update={update} errors={errors} />;
      case 6: return (
        <ReviewStep
          data={data}
          toml={toml}
          copied={copied}
          onCopy={copyToClipboard}
          onDownload={downloadToml}
          onRegenerate={() => setToml(generateToml(data))}
        />
      );
      default: return null;
    }
  };

  return (
    <div className="min-h-screen bg-gray-900 py-8 px-4">
      <div className="max-w-4xl mx-auto">
        {/* Header */}
        <div className="text-center mb-8">
          <h1 className="text-3xl font-bold text-gray-100 tracking-tight">GitSvnSync Setup</h1>
          <p className="text-gray-400 mt-2">Configure your SVN-to-Git synchronization in a few steps</p>
        </div>

        {/* Step Indicator */}
        <StepIndicator currentStep={step} steps={STEPS} onStepClick={(i) => { if (i < step) setStep(i); }} />

        {/* Step Content */}
        <div ref={stepRef} className="mt-8 bg-gray-800 rounded-xl shadow-lg border border-gray-700 p-8">
          {renderStep()}
        </div>

        {/* Navigation */}
        <div className="mt-6 flex items-center justify-between">
          <div>
            {step > 0 && (
              <button onClick={goBack} className="px-5 py-2.5 border border-gray-600 text-gray-300 hover:bg-gray-700 rounded-lg text-sm font-medium transition-colors">
                Back
              </button>
            )}
          </div>
          <div className="flex items-center space-x-3">
            {step === 0 && (
              <button onClick={() => navigate('/login')} className="px-5 py-2.5 text-gray-400 hover:text-gray-200 text-sm font-medium transition-colors">
                Skip to Login
              </button>
            )}
            {step < STEPS.length - 1 && (
              <button onClick={goNext} className="px-6 py-2.5 bg-blue-600 text-white hover:bg-blue-700 rounded-lg text-sm font-medium transition-colors shadow-sm">
                {step === 0 ? 'Get Started' : 'Next'}
              </button>
            )}
            {step === STEPS.length - 1 && (
              <button onClick={() => navigate('/login')} className="px-6 py-2.5 bg-emerald-600 text-white hover:bg-emerald-700 rounded-lg text-sm font-medium transition-colors shadow-sm">
                Go to Dashboard
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step Indicator
// ---------------------------------------------------------------------------

function StepIndicator({
  currentStep,
  steps,
  onStepClick,
}: {
  currentStep: number;
  steps: { label: string; short: string }[];
  onStepClick: (i: number) => void;
}) {
  return (
    <>
      {/* Desktop */}
      <div className="hidden md:flex items-center justify-between">
        {steps.map((s, i) => (
          <div key={i} className="flex items-center flex-1 last:flex-none">
            <button
              onClick={() => onStepClick(i)}
              disabled={i > currentStep}
              className="flex flex-col items-center group"
            >
              <div
                className={`w-9 h-9 rounded-full flex items-center justify-center text-sm font-semibold border-2 transition-all duration-200 ${
                  i < currentStep
                    ? 'bg-blue-600 border-blue-600 text-white'
                    : i === currentStep
                      ? 'border-blue-500 text-blue-400 bg-blue-500/10'
                      : 'border-gray-600 text-gray-500 bg-gray-800'
                }`}
              >
                {i < currentStep ? (
                  <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M5 13l4 4L19 7" />
                  </svg>
                ) : (
                  i + 1
                )}
              </div>
              <span
                className={`mt-1.5 text-xs font-medium transition-colors ${
                  i <= currentStep ? 'text-gray-300' : 'text-gray-500'
                }`}
              >
                {s.short}
              </span>
            </button>
            {i < steps.length - 1 && (
              <div
                className={`flex-1 h-0.5 mx-2 mt-[-1rem] transition-colors duration-200 ${
                  i < currentStep ? 'bg-blue-600' : 'bg-gray-700'
                }`}
              />
            )}
          </div>
        ))}
      </div>
      {/* Mobile */}
      <div className="md:hidden text-center">
        <span className="text-sm text-gray-400">
          Step {currentStep + 1} of {steps.length}:
        </span>
        <span className="ml-2 text-sm font-medium text-gray-200">{steps[currentStep].label}</span>
      </div>
    </>
  );
}

// ---------------------------------------------------------------------------
// Shared Form Components
// ---------------------------------------------------------------------------

function FormField({
  label,
  name,
  value,
  onChange,
  error,
  placeholder,
  help,
  type = 'text',
  mono,
  required,
  disabled,
}: {
  label: string;
  name: string;
  value: string | number;
  onChange: (val: string) => void;
  error?: string;
  placeholder?: string;
  help?: string;
  type?: string;
  mono?: boolean;
  required?: boolean;
  disabled?: boolean;
}) {
  return (
    <div>
      <label htmlFor={name} className="block text-sm font-medium text-gray-300 mb-1">
        {label}
        {required && <span className="text-red-400 ml-1">*</span>}
      </label>
      <input
        id={name}
        name={name}
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        disabled={disabled}
        className={`w-full rounded-lg border bg-gray-700 text-gray-100 placeholder-gray-500 px-3 py-2.5 text-sm transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500 ${
          error ? 'border-red-500' : 'border-gray-600'
        } ${mono ? 'font-mono' : ''} ${disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
        aria-describedby={error ? `${name}-error` : help ? `${name}-help` : undefined}
      />
      {error && (
        <p id={`${name}-error`} className="text-red-400 text-xs mt-1">{error}</p>
      )}
      {help && !error && (
        <p id={`${name}-help`} className="text-gray-500 text-xs mt-1">{help}</p>
      )}
    </div>
  );
}

function FormSelect({
  label,
  name,
  value,
  onChange,
  options,
  help,
}: {
  label: string;
  name: string;
  value: string;
  onChange: (val: string) => void;
  options: { value: string; label: string }[];
  help?: string;
}) {
  return (
    <div>
      <label htmlFor={name} className="block text-sm font-medium text-gray-300 mb-1">{label}</label>
      <select
        id={name}
        name={name}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full rounded-lg border border-gray-600 bg-gray-700 text-gray-100 px-3 py-2.5 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500"
      >
        {options.map(o => (
          <option key={o.value} value={o.value}>{o.label}</option>
        ))}
      </select>
      {help && <p className="text-gray-500 text-xs mt-1">{help}</p>}
    </div>
  );
}

function ToggleSwitch({
  label,
  checked,
  onChange,
  help,
}: {
  label: string;
  checked: boolean;
  onChange: (val: boolean) => void;
  help?: string;
}) {
  return (
    <div className="flex items-start space-x-3">
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        onClick={() => onChange(!checked)}
        className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors flex-shrink-0 mt-0.5 ${
          checked ? 'bg-blue-600' : 'bg-gray-600'
        }`}
      >
        <span
          className={`inline-block h-4 w-4 rounded-full bg-white transition-transform ${
            checked ? 'translate-x-6' : 'translate-x-1'
          }`}
        />
      </button>
      <div>
        <span className="text-sm font-medium text-gray-300">{label}</span>
        {help && <p className="text-gray-500 text-xs mt-0.5">{help}</p>}
      </div>
    </div>
  );
}

function SectionHeading({ title, description, color = 'blue' }: { title: string; description?: string; color?: string }) {
  const dotColors: Record<string, string> = {
    blue: 'bg-blue-400', green: 'bg-emerald-400', orange: 'bg-orange-400',
    purple: 'bg-purple-400', cyan: 'bg-cyan-400', red: 'bg-red-400',
  };
  return (
    <div className="mb-5">
      <h2 className="text-lg font-semibold text-gray-100 flex items-center space-x-2">
        <span className={`w-2 h-2 rounded-full ${dotColors[color] ?? dotColors.blue}`} />
        <span>{title}</span>
      </h2>
      {description && <p className="text-sm text-gray-400 mt-1 ml-4">{description}</p>}
    </div>
  );
}

function TestConnectionButton({ onTest }: { onTest: () => Promise<{ ok: boolean; message: string }> }) {
  const [state, setState] = useState<'idle' | 'loading' | 'success' | 'error'>('idle');
  const [msg, setMsg] = useState('');

  const handleTest = async () => {
    setState('loading');
    try {
      const result = await onTest();
      setState(result.ok ? 'success' : 'error');
      setMsg(result.message);
    } catch (e: unknown) {
      setState('error');
      setMsg(e instanceof Error ? e.message : 'Connection test failed');
    }
    setTimeout(() => setState('idle'), 5000);
  };

  return (
    <div className="flex items-center space-x-3">
      <button
        onClick={handleTest}
        disabled={state === 'loading'}
        className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
          state === 'success'
            ? 'bg-emerald-600/20 text-emerald-400 border border-emerald-600'
            : state === 'error'
              ? 'bg-red-600/20 text-red-400 border border-red-600'
              : 'bg-gray-700 text-gray-300 border border-gray-600 hover:bg-gray-600'
        } disabled:opacity-50`}
      >
        {state === 'loading' ? (
          <span className="flex items-center space-x-2">
            <svg className="w-4 h-4 animate-spin" viewBox="0 0 24 24" fill="none">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
            <span>Testing...</span>
          </span>
        ) : state === 'success' ? (
          'Connected'
        ) : state === 'error' ? (
          'Failed'
        ) : (
          'Test Connection'
        )}
      </button>
      {msg && (state === 'success' || state === 'error') && (
        <span className={`text-xs ${state === 'success' ? 'text-emerald-400' : 'text-red-400'}`}>{msg}</span>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 0: Welcome
// ---------------------------------------------------------------------------

function WelcomeStep() {
  return (
    <div className="text-center py-6">
      <div className="inline-flex items-center justify-center w-20 h-20 rounded-2xl bg-blue-600/10 border border-blue-500/30 mb-6">
        <svg className="w-10 h-10 text-blue-400" viewBox="0 0 48 48" fill="none">
          <rect x="2" y="2" width="44" height="44" rx="10" stroke="currentColor" strokeWidth="2.5" fill="none" />
          <path d="M14 24 L22 32 L34 16" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round" />
          <circle cx="14" cy="16" r="3" fill="currentColor" opacity="0.5" />
          <circle cx="34" cy="32" r="3" fill="currentColor" opacity="0.5" />
        </svg>
      </div>
      <h2 className="text-2xl font-bold text-gray-100 mb-3">Welcome to GitSvnSync</h2>
      <p className="text-gray-400 max-w-lg mx-auto leading-relaxed">
        This wizard will guide you through setting up bidirectional synchronization
        between your SVN repository and Git. At the end, you'll get a configuration
        file ready to deploy on your server.
      </p>
      <div className="mt-8 grid grid-cols-1 sm:grid-cols-3 gap-4 max-w-xl mx-auto text-left">
        <div className="bg-gray-700/50 rounded-lg p-4 border border-gray-600/50">
          <div className="text-blue-400 font-semibold text-sm mb-1">SVN & Git</div>
          <p className="text-gray-400 text-xs">Connect your SVN repo and Git provider (GitHub/Gitea)</p>
        </div>
        <div className="bg-gray-700/50 rounded-lg p-4 border border-gray-600/50">
          <div className="text-purple-400 font-semibold text-sm mb-1">Identity</div>
          <p className="text-gray-400 text-xs">Map SVN usernames to Git author identities</p>
        </div>
        <div className="bg-gray-700/50 rounded-lg p-4 border border-gray-600/50">
          <div className="text-emerald-400 font-semibold text-sm mb-1">Generate</div>
          <p className="text-gray-400 text-xs">Get a ready-to-deploy TOML configuration file</p>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 1: SVN
// ---------------------------------------------------------------------------

function SvnStep({
  data,
  update,
  errors,
}: {
  data: WizardData;
  update: (f: Partial<WizardData>) => void;
  errors: Errors;
}) {
  return (
    <div className="space-y-6">
      <SectionHeading
        title="SVN Repository"
        description="Configure the connection to your Subversion repository."
        color="orange"
      />
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="md:col-span-2">
          <FormField
            label="Repository URL"
            name="svn_url"
            value={data.svn_url}
            onChange={(v) => update({ svn_url: v })}
            error={errors.svn_url}
            placeholder="svn://svn.example.com/repos/project"
            help="The root URL of your SVN repository"
            mono
            required
          />
        </div>
        <FormField
          label="Username"
          name="svn_username"
          value={data.svn_username}
          onChange={(v) => update({ svn_username: v })}
          error={errors.svn_username}
          placeholder="sync-service"
          help="Service account username for SVN access"
          required
        />
        <FormField
          label="Password Environment Variable"
          name="svn_password_env"
          value={data.svn_password_env}
          onChange={(v) => update({ svn_password_env: v })}
          error={errors.svn_password_env}
          placeholder="GITSVNSYNC_SVN_PASSWORD"
          help="Name of the env var that holds the SVN password"
          mono
          required
        />
      </div>

      <div className="border-t border-gray-700 pt-5">
        <FormSelect
          label="Repository Layout"
          name="svn_layout"
          value={data.svn_layout}
          onChange={(v) => update({ svn_layout: v as 'standard' | 'custom' })}
          options={[
            { value: 'standard', label: 'Standard (trunk/branches/tags)' },
            { value: 'custom', label: 'Custom paths' },
          ]}
          help="Standard layout uses trunk/, branches/, and tags/ under the repo root"
        />
        <div className={`mt-4 grid grid-cols-1 md:grid-cols-3 gap-4 ${data.svn_layout === 'standard' ? 'opacity-50' : ''}`}>
          <FormField
            label="Trunk Path"
            name="svn_trunk_path"
            value={data.svn_trunk_path}
            onChange={(v) => update({ svn_trunk_path: v })}
            error={errors.svn_trunk_path}
            mono
          />
          <FormField
            label="Branches Path"
            name="svn_branches_path"
            value={data.svn_branches_path}
            onChange={(v) => update({ svn_branches_path: v })}
            disabled={data.svn_layout === 'standard'}
            mono
          />
          <FormField
            label="Tags Path"
            name="svn_tags_path"
            value={data.svn_tags_path}
            onChange={(v) => update({ svn_tags_path: v })}
            disabled={data.svn_layout === 'standard'}
            mono
          />
        </div>
      </div>

      <div className="border-t border-gray-700 pt-5">
        <TestConnectionButton
          onTest={() => api.testSvnConnection({ url: data.svn_url, username: data.svn_username })}
        />
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 2: Git Provider
// ---------------------------------------------------------------------------

function GitStep({
  data,
  update,
  errors,
}: {
  data: WizardData;
  update: (f: Partial<WizardData>) => void;
  errors: Errors;
}) {
  const handleProviderChange = (provider: string) => {
    const p = provider as 'github' | 'gitea';
    const apiUrl = p === 'github' ? 'https://api.github.com' : data.git_api_url;
    update({ git_provider: p, git_api_url: apiUrl });
  };

  return (
    <div className="space-y-6">
      <SectionHeading
        title="Git Provider"
        description="Configure the connection to GitHub or Gitea."
        color="purple"
      />

      {/* Provider selection */}
      <div className="flex space-x-3">
        {(['github', 'gitea'] as const).map(p => (
          <button
            key={p}
            onClick={() => handleProviderChange(p)}
            className={`flex-1 py-3 px-4 rounded-lg border text-sm font-medium transition-all ${
              data.git_provider === p
                ? 'border-blue-500 bg-blue-600/10 text-blue-400'
                : 'border-gray-600 bg-gray-700/50 text-gray-400 hover:bg-gray-700'
            }`}
          >
            {p === 'github' ? 'GitHub / GitHub Enterprise' : 'Gitea'}
          </button>
        ))}
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="md:col-span-2">
          <FormField
            label="API URL"
            name="git_api_url"
            value={data.git_api_url}
            onChange={(v) => update({ git_api_url: v })}
            error={errors.git_api_url}
            placeholder={data.git_provider === 'github' ? 'https://api.github.com' : 'http://gitea.example.com:3000/api/v1'}
            help={data.git_provider === 'github' ? 'Use https://api.github.com for GitHub.com, or your GHE API URL' : 'Your Gitea server API URL (e.g., http://host:3000/api/v1)'}
            mono
            required
          />
        </div>
        <FormField
          label="Repository"
          name="git_repo"
          value={data.git_repo}
          onChange={(v) => update({ git_repo: v })}
          error={errors.git_repo}
          placeholder="owner/repo-name"
          help="In owner/repo format"
          mono
          required
        />
        <FormField
          label="Default Branch"
          name="git_default_branch"
          value={data.git_default_branch}
          onChange={(v) => update({ git_default_branch: v })}
          placeholder="main"
          mono
        />
        <div className="md:col-span-2">
          <FormField
            label="Token Environment Variable"
            name="git_token_env"
            value={data.git_token_env}
            onChange={(v) => update({ git_token_env: v })}
            error={errors.git_token_env}
            placeholder="GITSVNSYNC_GITHUB_TOKEN"
            help="Name of the env var that holds the API token"
            mono
            required
          />
        </div>
      </div>

      <div className="border-t border-gray-700 pt-5">
        <TestConnectionButton
          onTest={() => api.testGitConnection({ api_url: data.git_api_url, repo: data.git_repo, provider: data.git_provider })}
        />
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 3: Sync Settings
// ---------------------------------------------------------------------------

function SyncStep({
  data,
  update,
}: {
  data: WizardData;
  update: (f: Partial<WizardData>) => void;
}) {
  return (
    <div className="space-y-6">
      <SectionHeading
        title="Sync Settings"
        description="Configure how commits are synchronized between SVN and Git."
        color="cyan"
      />

      {/* Mode selection */}
      <div>
        <label className="block text-sm font-medium text-gray-300 mb-2">Sync Mode</label>
        <div className="flex space-x-3">
          <button
            onClick={() => update({ sync_mode: 'direct' })}
            className={`flex-1 py-3 px-4 rounded-lg border text-left transition-all ${
              data.sync_mode === 'direct'
                ? 'border-blue-500 bg-blue-600/10'
                : 'border-gray-600 bg-gray-700/50 hover:bg-gray-700'
            }`}
          >
            <div className={`text-sm font-medium ${data.sync_mode === 'direct' ? 'text-blue-400' : 'text-gray-300'}`}>
              Direct Push
            </div>
            <p className="text-xs text-gray-400 mt-1">Commits are pushed directly to the target branch</p>
          </button>
          <button
            onClick={() => update({ sync_mode: 'pr' })}
            className={`flex-1 py-3 px-4 rounded-lg border text-left transition-all ${
              data.sync_mode === 'pr'
                ? 'border-blue-500 bg-blue-600/10'
                : 'border-gray-600 bg-gray-700/50 hover:bg-gray-700'
            }`}
          >
            <div className={`text-sm font-medium ${data.sync_mode === 'pr' ? 'text-blue-400' : 'text-gray-300'}`}>
              Pull Request
            </div>
            <p className="text-xs text-gray-400 mt-1">Changes create a PR for review before merging</p>
          </button>
        </div>
      </div>

      <div className="space-y-4">
        <ToggleSwitch
          label="Auto Merge"
          checked={data.sync_auto_merge}
          onChange={(v) => update({ sync_auto_merge: v })}
          help="Automatically resolve 3-way merge conflicts when possible"
        />
        <ToggleSwitch
          label="Sync Tags"
          checked={data.sync_tags}
          onChange={(v) => update({ sync_tags: v })}
          help="Synchronize SVN tags to Git tags and vice versa"
        />
      </div>

      {/* PR-specific settings */}
      {data.sync_mode === 'pr' && (
        <div className="border-t border-gray-700 pt-5 space-y-4">
          <h3 className="text-sm font-semibold text-gray-200">Pull Request Settings</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <FormField
              label="PR Title Prefix"
              name="pr_title_prefix"
              value={data.pr_title_prefix}
              onChange={(v) => update({ pr_title_prefix: v })}
              placeholder="[svn-sync]"
            />
            <FormField
              label="Labels"
              name="pr_labels"
              value={data.pr_labels}
              onChange={(v) => update({ pr_labels: v })}
              placeholder="svn-sync, automated"
              help="Comma-separated list"
            />
            <FormField
              label="Reviewers"
              name="pr_reviewers"
              value={data.pr_reviewers}
              onChange={(v) => update({ pr_reviewers: v })}
              placeholder="username1, username2"
              help="Comma-separated GitHub usernames"
            />
            <div className="flex items-end">
              <ToggleSwitch
                label="Auto-merge PRs"
                checked={data.pr_auto_merge}
                onChange={(v) => update({ pr_auto_merge: v })}
                help="Automatically merge PRs when checks pass"
              />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 4: Identity
// ---------------------------------------------------------------------------

function IdentityStep({
  data,
  update,
  errors,
}: {
  data: WizardData;
  update: (f: Partial<WizardData>) => void;
  errors: Errors;
}) {
  const [newMapping, setNewMapping] = useState<AuthorMapping>({ svn_username: '', name: '', email: '' });

  const addMapping = () => {
    if (!newMapping.svn_username || !newMapping.name || !newMapping.email) return;
    update({ identity_mappings: [...data.identity_mappings, { ...newMapping }] });
    setNewMapping({ svn_username: '', name: '', email: '' });
  };

  const removeMapping = (svnUsername: string) => {
    update({ identity_mappings: data.identity_mappings.filter(m => m.svn_username !== svnUsername) });
  };

  return (
    <div className="space-y-6">
      <SectionHeading
        title="Identity Mapping"
        description="Map SVN usernames to Git author identities so commits are attributed correctly."
        color="green"
      />

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <FormField
          label="Fallback Email Domain"
          name="identity_email_domain"
          value={data.identity_email_domain}
          onChange={(v) => update({ identity_email_domain: v })}
          error={errors.identity_email_domain}
          placeholder="company.com"
          help="Unmapped users get email: svn-username@domain"
        />
        <FormField
          label="Author Mapping File"
          name="identity_mapping_file"
          value={data.identity_mapping_file}
          onChange={(v) => update({ identity_mapping_file: v })}
          placeholder="/etc/gitsvnsync/authors.toml"
          help="Optional: path to a TOML file with explicit mappings"
          mono
        />
      </div>

      {/* Inline mappings table */}
      <div className="border-t border-gray-700 pt-5">
        <h3 className="text-sm font-semibold text-gray-200 mb-3">Inline Author Mappings</h3>
        {data.identity_mappings.length > 0 && (
          <div className="overflow-x-auto mb-4">
            <table className="min-w-full divide-y divide-gray-700">
              <thead>
                <tr>
                  <th className="px-3 py-2 text-left text-xs font-medium text-gray-400 uppercase">SVN Username</th>
                  <th className="px-3 py-2 text-left text-xs font-medium text-gray-400 uppercase">Git Name</th>
                  <th className="px-3 py-2 text-left text-xs font-medium text-gray-400 uppercase">Email</th>
                  <th className="px-3 py-2 w-16"></th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-700">
                {data.identity_mappings.map(m => (
                  <tr key={m.svn_username} className="hover:bg-gray-700/50">
                    <td className="px-3 py-2 text-sm font-mono text-gray-200">{m.svn_username}</td>
                    <td className="px-3 py-2 text-sm text-gray-300">{m.name}</td>
                    <td className="px-3 py-2 text-sm text-gray-300">{m.email}</td>
                    <td className="px-3 py-2 text-right">
                      <button onClick={() => removeMapping(m.svn_username)} className="text-red-400 hover:text-red-300 text-xs">
                        Remove
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        <div className="flex space-x-2">
          <input
            placeholder="SVN username"
            value={newMapping.svn_username}
            onChange={(e) => setNewMapping({ ...newMapping, svn_username: e.target.value })}
            className="flex-1 rounded-lg border border-gray-600 bg-gray-700 text-gray-100 placeholder-gray-500 px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500 focus:outline-none font-mono"
          />
          <input
            placeholder="Full Name"
            value={newMapping.name}
            onChange={(e) => setNewMapping({ ...newMapping, name: e.target.value })}
            className="flex-1 rounded-lg border border-gray-600 bg-gray-700 text-gray-100 placeholder-gray-500 px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500 focus:outline-none"
          />
          <input
            placeholder="email@company.com"
            value={newMapping.email}
            onChange={(e) => setNewMapping({ ...newMapping, email: e.target.value })}
            className="flex-1 rounded-lg border border-gray-600 bg-gray-700 text-gray-100 placeholder-gray-500 px-3 py-2 text-sm focus:ring-2 focus:ring-blue-500/50 focus:border-blue-500 focus:outline-none"
          />
          <button
            onClick={addMapping}
            disabled={!newMapping.svn_username || !newMapping.name || !newMapping.email}
            className="px-4 py-2 bg-blue-600 text-white rounded-lg text-sm font-medium hover:bg-blue-700 disabled:opacity-50 transition-colors"
          >
            Add
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 5: Server & Auth
// ---------------------------------------------------------------------------

function ServerAuthStep({
  data,
  update,
  errors,
}: {
  data: WizardData;
  update: (f: Partial<WizardData>) => void;
  errors: Errors;
}) {
  return (
    <div className="space-y-8">
      {/* Web Server */}
      <div>
        <SectionHeading title="Web Server" description="Configure the dashboard web server." color="cyan" />
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <FormField
            label="Listen Address"
            name="web_listen"
            value={data.web_listen}
            onChange={(v) => update({ web_listen: v })}
            error={errors.web_listen}
            placeholder="0.0.0.0:8080"
            help="host:port to bind the web server"
            mono
            required
          />
          <FormSelect
            label="Authentication Mode"
            name="web_auth_mode"
            value={data.web_auth_mode}
            onChange={(v) => update({ web_auth_mode: v as 'simple' | 'github_oauth' | 'both' })}
            options={[
              { value: 'simple', label: 'Simple (password)' },
              { value: 'github_oauth', label: 'GitHub OAuth' },
              { value: 'both', label: 'Both' },
            ]}
          />
        </div>
        {/* Auth-specific fields */}
        {(data.web_auth_mode === 'simple' || data.web_auth_mode === 'both') && (
          <div className="mt-4">
            <FormField
              label="Admin Password Env Var"
              name="web_admin_password_env"
              value={data.web_admin_password_env}
              onChange={(v) => update({ web_admin_password_env: v })}
              error={errors.web_admin_password_env}
              placeholder="GITSVNSYNC_ADMIN_PASSWORD"
              mono
              required
            />
          </div>
        )}
        {(data.web_auth_mode === 'github_oauth' || data.web_auth_mode === 'both') && (
          <div className="mt-4 grid grid-cols-1 md:grid-cols-2 gap-4">
            <FormField
              label="OAuth Client ID"
              name="web_oauth_client_id"
              value={data.web_oauth_client_id}
              onChange={(v) => update({ web_oauth_client_id: v })}
              mono
            />
            <FormField
              label="OAuth Client Secret Env Var"
              name="web_oauth_client_secret_env"
              value={data.web_oauth_client_secret_env}
              onChange={(v) => update({ web_oauth_client_secret_env: v })}
              mono
            />
            <div className="md:col-span-2">
              <FormField
                label="Allowed GitHub Users"
                name="web_oauth_allowed_users"
                value={data.web_oauth_allowed_users}
                onChange={(v) => update({ web_oauth_allowed_users: v })}
                placeholder="user1, user2"
                help="Comma-separated list of GitHub usernames"
              />
            </div>
          </div>
        )}
      </div>

      {/* Daemon */}
      <div className="border-t border-gray-700 pt-6">
        <SectionHeading title="Daemon Settings" description="Configure the sync daemon process." color="green" />
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <FormField
            label="Poll Interval (seconds)"
            name="daemon_poll_interval"
            value={data.daemon_poll_interval}
            onChange={(v) => update({ daemon_poll_interval: parseInt(v) || 0 })}
            error={errors.daemon_poll_interval}
            type="number"
            required
          />
          <FormSelect
            label="Log Level"
            name="daemon_log_level"
            value={data.daemon_log_level}
            onChange={(v) => update({ daemon_log_level: v })}
            options={[
              { value: 'trace', label: 'Trace' },
              { value: 'debug', label: 'Debug' },
              { value: 'info', label: 'Info' },
              { value: 'warn', label: 'Warn' },
              { value: 'error', label: 'Error' },
            ]}
          />
          <FormField
            label="Data Directory"
            name="daemon_data_dir"
            value={data.daemon_data_dir}
            onChange={(v) => update({ daemon_data_dir: v })}
            error={errors.daemon_data_dir}
            placeholder="/var/lib/gitsvnsync"
            mono
            required
          />
        </div>
      </div>

      {/* Notifications */}
      <div className="border-t border-gray-700 pt-6">
        <SectionHeading title="Notifications" description="Optional: configure Slack or email alerts." color="orange" />
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="md:col-span-2">
            <FormField
              label="Slack Webhook Env Var"
              name="notif_slack_webhook_env"
              value={data.notif_slack_webhook_env}
              onChange={(v) => update({ notif_slack_webhook_env: v })}
              placeholder="GITSVNSYNC_SLACK_WEBHOOK"
              help="Leave empty to disable Slack notifications"
              mono
            />
          </div>
          <FormField
            label="Email SMTP Server"
            name="notif_email_smtp"
            value={data.notif_email_smtp}
            onChange={(v) => update({ notif_email_smtp: v })}
            placeholder="smtp.company.com:587"
            mono
          />
          <FormField
            label="From Address"
            name="notif_email_from"
            value={data.notif_email_from}
            onChange={(v) => update({ notif_email_from: v })}
            placeholder="gitsvnsync@company.com"
          />
          <div className="md:col-span-2">
            <FormField
              label="Email Recipients"
              name="notif_email_recipients"
              value={data.notif_email_recipients}
              onChange={(v) => update({ notif_email_recipients: v })}
              placeholder="admin@company.com, ops@company.com"
              help="Comma-separated list"
            />
          </div>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Step 6: Review & Generate
// ---------------------------------------------------------------------------

function ReviewStep({
  data,
  toml,
  copied,
  onCopy,
  onDownload,
  onRegenerate,
}: {
  data: WizardData;
  toml: string;
  copied: boolean;
  onCopy: () => void;
  onDownload: () => void;
  onRegenerate: () => void;
}) {
  const [savingMappings, setSavingMappings] = useState(false);
  const [mappingsSaved, setMappingsSaved] = useState(false);

  const saveMappings = async () => {
    setSavingMappings(true);
    try {
      await api.updateIdentityMappings(data.identity_mappings);
      setMappingsSaved(true);
    } catch {
      // silently fail if not authed
    }
    setSavingMappings(false);
  };

  return (
    <div className="space-y-6">
      <SectionHeading title="Review & Generate" description="Review your configuration and download the TOML file." color="blue" />

      {/* Summary cards */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <SummaryCard
          title="SVN"
          color="orange"
          items={[
            ['URL', data.svn_url],
            ['Username', data.svn_username],
            ['Layout', data.svn_layout],
            ['Trunk', data.svn_trunk_path],
          ]}
        />
        <SummaryCard
          title="Git"
          color="purple"
          items={[
            ['Provider', data.git_provider],
            ['API URL', data.git_api_url],
            ['Repository', data.git_repo],
            ['Branch', data.git_default_branch],
          ]}
        />
        <SummaryCard
          title="Sync"
          color="cyan"
          items={[
            ['Mode', data.sync_mode],
            ['Auto Merge', data.sync_auto_merge ? 'Yes' : 'No'],
            ['Sync Tags', data.sync_tags ? 'Yes' : 'No'],
          ]}
        />
        <SummaryCard
          title="Server"
          color="green"
          items={[
            ['Listen', data.web_listen],
            ['Auth', data.web_auth_mode],
            ['Poll Interval', `${data.daemon_poll_interval}s`],
            ['Log Level', data.daemon_log_level],
          ]}
        />
      </div>

      {data.identity_mappings.length > 0 && (
        <div className="flex items-center justify-between bg-gray-700/30 rounded-lg p-4 border border-gray-700">
          <div>
            <span className="text-sm text-gray-300">
              {data.identity_mappings.length} identity mapping{data.identity_mappings.length !== 1 ? 's' : ''} configured
            </span>
          </div>
          <button
            onClick={saveMappings}
            disabled={savingMappings || mappingsSaved}
            className={`px-4 py-2 rounded-lg text-sm font-medium transition-colors ${
              mappingsSaved
                ? 'bg-emerald-600/20 text-emerald-400 border border-emerald-600'
                : 'bg-blue-600 text-white hover:bg-blue-700'
            } disabled:opacity-50`}
          >
            {mappingsSaved ? 'Saved!' : savingMappings ? 'Saving...' : 'Save to Database'}
          </button>
        </div>
      )}

      {/* TOML output */}
      <div className="border-t border-gray-700 pt-5">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-sm font-semibold text-gray-200">Generated Configuration</h3>
          <div className="flex space-x-2">
            <button onClick={onRegenerate} className="px-3 py-1.5 text-xs text-gray-400 hover:text-gray-200 border border-gray-600 rounded-lg hover:bg-gray-700 transition-colors">
              Regenerate
            </button>
            <button
              onClick={onCopy}
              className={`px-3 py-1.5 text-xs rounded-lg border transition-colors ${
                copied
                  ? 'border-emerald-600 text-emerald-400 bg-emerald-600/10'
                  : 'border-gray-600 text-gray-300 hover:bg-gray-700'
              }`}
            >
              {copied ? 'Copied!' : 'Copy'}
            </button>
            <button onClick={onDownload} className="px-3 py-1.5 text-xs bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors">
              Download
            </button>
          </div>
        </div>
        <pre className="bg-gray-900 border border-gray-700 rounded-lg p-5 text-sm font-mono text-gray-300 overflow-x-auto max-h-[500px] overflow-y-auto leading-relaxed whitespace-pre">
          {toml}
        </pre>
      </div>

      {/* Next steps */}
      <div className="bg-blue-900/20 border border-blue-700/50 rounded-lg p-5">
        <h3 className="text-sm font-semibold text-blue-300 mb-2">Next Steps</h3>
        <ol className="text-sm text-gray-400 space-y-1 list-decimal list-inside">
          <li>Save the TOML file to your server (e.g., <code className="text-gray-300 font-mono text-xs">/etc/gitsvnsync/config.toml</code>)</li>
          <li>Set the required environment variables on the server</li>
          <li>Start the daemon: <code className="text-gray-300 font-mono text-xs">gitsvnsync-daemon --config /path/to/config.toml</code></li>
          <li>Open the dashboard at <code className="text-gray-300 font-mono text-xs">http://your-server:{data.web_listen.split(':')[1] || '8080'}</code></li>
        </ol>
      </div>
    </div>
  );
}

function SummaryCard({ title, color, items }: { title: string; color: string; items: [string, string][] }) {
  const borderColors: Record<string, string> = {
    orange: 'border-orange-700/50', purple: 'border-purple-700/50',
    cyan: 'border-cyan-700/50', green: 'border-emerald-700/50',
    blue: 'border-blue-700/50',
  };
  const titleColors: Record<string, string> = {
    orange: 'text-orange-400', purple: 'text-purple-400',
    cyan: 'text-cyan-400', green: 'text-emerald-400',
    blue: 'text-blue-400',
  };

  return (
    <div className={`bg-gray-700/30 rounded-lg p-4 border ${borderColors[color] ?? 'border-gray-700'}`}>
      <h4 className={`text-sm font-semibold mb-2 ${titleColors[color] ?? 'text-gray-300'}`}>{title}</h4>
      <dl className="space-y-1.5">
        {items.map(([label, value]) => (
          <div key={label} className="flex justify-between text-xs">
            <dt className="text-gray-500">{label}</dt>
            <dd className="text-gray-300 font-mono truncate ml-3 max-w-[60%] text-right">{value || '-'}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
