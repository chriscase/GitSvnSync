import { useQuery } from '@tanstack/react-query';
import { Clock, ArrowRight, Terminal, CheckCircle2 } from 'lucide-react';
import { api, type ImportStatus } from '../api';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatElapsed(startedAt: string | null, endedAt?: string | null): string {
  if (!startedAt) return '--:--';
  const start = new Date(startedAt).getTime();
  const end = endedAt ? new Date(endedAt).getTime() : Date.now();
  const secs = Math.max(0, Math.floor((end - start) / 1000));
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

function phaseLabel(phase: string): string {
  const labels: Record<string, string> = {
    idle: 'Idle',
    connecting: 'Connecting',
    importing: 'Importing',
    verifying: 'Verifying',
    final_push: 'Final Push',
    completed: 'Completed',
    failed: 'Failed',
    cancelled: 'Cancelled',
  };
  return labels[phase] ?? phase;
}

function phaseDotColor(phase: string): string {
  if (phase === 'completed') return 'bg-emerald-400';
  if (phase === 'failed') return 'bg-red-400';
  if (phase === 'cancelled') return 'bg-yellow-400';
  if (phase === 'idle') return 'bg-gray-500';
  return 'bg-blue-400 animate-pulse';
}

// The five import phases in order
const PHASE_STEPS = [
  { key: 'connecting', label: 'Connect' },
  { key: 'importing', label: 'Import' },
  { key: 'verifying', label: 'Verify' },
  { key: 'final_push', label: 'Push' },
  { key: 'completed', label: 'Complete' },
] as const;

function phaseIndex(phase: string): number {
  const idx = PHASE_STEPS.findIndex((s) => s.key === phase);
  return idx >= 0 ? idx : -1;
}

// ---------------------------------------------------------------------------
// Phase Dots
// ---------------------------------------------------------------------------

function PhaseDots({ phase }: { phase: string }) {
  const currentIdx = phaseIndex(phase);
  const isFailed = phase === 'failed';
  const isCancelled = phase === 'cancelled';

  return (
    <div className="flex items-center space-x-1">
      {PHASE_STEPS.map((step, i) => {
        let dotClass = 'bg-gray-600'; // future
        if (isFailed || isCancelled) {
          dotClass = i <= currentIdx ? (isFailed ? 'bg-red-400' : 'bg-yellow-400') : 'bg-gray-600';
        } else if (i < currentIdx) {
          dotClass = 'bg-emerald-400'; // past
        } else if (i === currentIdx) {
          dotClass = 'bg-blue-400 animate-pulse'; // current
          if (phase === 'completed') dotClass = 'bg-emerald-400';
        }

        return (
          <div key={step.key} className="flex flex-col items-center">
            <div className={`w-2 h-2 rounded-full ${dotClass}`} title={step.label} />
            <span className="text-[9px] text-gray-500 mt-0.5 leading-none">{step.label}</span>
          </div>
        );
      })}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Component (self-fetching)
// ---------------------------------------------------------------------------

export default function ImportProgressCard() {
  const { data: status } = useQuery<ImportStatus>({
    queryKey: ['import-status'],
    queryFn: api.getImportStatus,
    refetchInterval: 2000,
    refetchIntervalInBackground: false,
  });

  // No data yet from API
  if (!status) {
    return (
      <div className="bg-gray-800 border border-gray-700 rounded-xl p-5 shadow-lg">
        <div className="text-sm text-gray-500 italic">Loading import status...</div>
      </div>
    );
  }

  // Never ran an import
  const neverRan = status.phase === 'idle' && !status.started_at;
  if (neverRan) {
    return (
      <div className="bg-gray-800 border border-gray-700 rounded-xl p-5 shadow-lg">
        <div className="flex items-center justify-between">
          <div>
            <h3 className="text-sm font-semibold text-gray-200">SVN Import</h3>
            <p className="text-xs text-gray-500 mt-1">No import history</p>
          </div>
          <a
            href="/repos"
            className="flex items-center space-x-1 text-sm text-blue-400 hover:text-blue-300 transition-colors"
          >
            <span>Manage Repos</span>
            <ArrowRight className="w-4 h-4" />
          </a>
        </div>
      </div>
    );
  }

  // Completed state — show success summary
  if (status.phase === 'completed') {
    return (
      <div className="bg-gray-800 border border-gray-700 rounded-xl p-5 shadow-lg">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center space-x-2">
            <CheckCircle2 className="w-5 h-5 text-emerald-400" />
            <h3 className="text-sm font-semibold text-gray-200">SVN Import Complete</h3>
          </div>
          <div className="flex items-center space-x-1 text-xs text-gray-500">
            <Clock className="w-3.5 h-3.5" />
            <span>{formatElapsed(status.started_at, status.completed_at)}</span>
          </div>
        </div>
        <div className="w-full h-2 bg-gray-700 rounded-full overflow-hidden mb-3">
          <div className="h-full rounded-full bg-emerald-500 w-full" />
        </div>
        <div className="grid grid-cols-4 gap-3 mb-3">
          <StatCell label="Revisions" value={`${status.total_revs}`} />
          <StatCell label="Commits" value={`${status.commits_created}`} />
          <StatCell label="Batches" value={`${status.batches_pushed}`} />
          <StatCell label="LFS Files" value={`${status.lfs_unique_count}`} />
        </div>
        <a
          href="/"
          className="flex items-center justify-center space-x-1 text-sm text-blue-400 hover:text-blue-300 transition-colors"
        >
          <span>View Full Import</span>
          <ArrowRight className="w-4 h-4" />
        </a>
      </div>
    );
  }

  // Active / failed / cancelled — full card
  const percentage =
    status.total_revs > 0
      ? Math.round((status.current_rev / status.total_revs) * 100)
      : 0;

  const barColor =
    status.phase === 'failed'
      ? 'bg-red-500'
      : status.phase === 'cancelled'
        ? 'bg-yellow-500'
        : 'bg-blue-500';

  const lastLogLines = (status.log_lines ?? []).slice(-5);

  return (
    <div className="bg-gray-800 border border-gray-700 rounded-xl p-5 shadow-lg">
      {/* Header row */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center space-x-2">
          <span className={`w-2.5 h-2.5 rounded-full ${phaseDotColor(status.phase)}`} />
          <h3 className="text-sm font-semibold text-gray-200">SVN Import</h3>
          <span className="text-xs text-gray-400 bg-gray-700 px-2 py-0.5 rounded-full">
            {phaseLabel(status.phase)}
          </span>
        </div>
        <div className="flex items-center space-x-1 text-xs text-gray-500">
          <Clock className="w-3.5 h-3.5" />
          <span>{formatElapsed(status.started_at)}</span>
        </div>
      </div>

      {/* Phase dots */}
      <div className="flex justify-center mb-4">
        <PhaseDots phase={status.phase} />
      </div>

      {/* Progress bar */}
      <div className="w-full h-2 bg-gray-700 rounded-full overflow-hidden mb-1">
        <div
          className={`h-full rounded-full transition-all duration-500 ease-out ${barColor}`}
          style={{ width: `${percentage}%` }}
        />
      </div>
      <div className="text-right text-xs text-gray-500 mb-3 font-mono">
        {status.current_rev} / {status.total_revs}
      </div>

      {/* Stats row */}
      <div className="grid grid-cols-4 gap-3 mb-4">
        <StatCell label="Revisions" value={`${status.current_rev}/${status.total_revs}`} />
        <StatCell label="Commits" value={`${status.commits_created}`} />
        <StatCell label="Batches" value={`${status.batches_pushed}`} />
        <StatCell label="LFS Files" value={`${status.lfs_unique_count}`} />
      </div>

      {/* Mini terminal */}
      {lastLogLines.length > 0 && (
        <div className="bg-gray-950 border border-gray-700 rounded-lg p-3 mb-4">
          <div className="flex items-center space-x-1.5 mb-2">
            <Terminal className="w-3 h-3 text-gray-500" />
            <span className="text-[10px] text-gray-500 uppercase tracking-wider">Log</span>
          </div>
          <div className="space-y-0.5 font-mono text-xs leading-relaxed max-h-[100px] overflow-y-auto">
            {lastLogLines.map((line, i) => (
              <div key={i} className="text-gray-400 truncate">{line}</div>
            ))}
          </div>
        </div>
      )}

      {/* Link */}
      <a
        href="/"
        className="flex items-center justify-center space-x-1 text-sm text-blue-400 hover:text-blue-300 transition-colors"
      >
        <span>View Full Import</span>
        <ArrowRight className="w-4 h-4" />
      </a>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function StatCell({ label, value }: { label: string; value: string }) {
  return (
    <div className="text-center">
      <div className="text-base font-bold text-gray-100 font-mono">{value}</div>
      <div className="text-[10px] text-gray-500 uppercase tracking-wider">{label}</div>
    </div>
  );
}
