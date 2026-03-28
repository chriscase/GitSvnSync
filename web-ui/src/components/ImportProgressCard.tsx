import { Clock, ArrowRight, Terminal } from 'lucide-react';
import type { ImportStatus } from '../api';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatElapsed(startedAt: string | null): string {
  if (!startedAt) return '--:--';
  const start = new Date(startedAt).getTime();
  const now = Date.now();
  const secs = Math.max(0, Math.floor((now - start) / 1000));
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

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function ImportProgressCard({
  status,
  linkTo = '/setup',
}: {
  status: ImportStatus;
  linkTo?: string;
}) {
  const percentage =
    status.total_revs > 0
      ? Math.round((status.current_rev / status.total_revs) * 100)
      : 0;

  const barColor =
    status.phase === 'completed'
      ? 'bg-emerald-500'
      : status.phase === 'failed'
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

      {/* Progress bar */}
      <div className="w-full h-2 bg-gray-700 rounded-full overflow-hidden mb-3">
        <div
          className={`h-full rounded-full transition-all duration-500 ease-out ${barColor}`}
          style={{ width: `${status.phase === 'completed' ? 100 : percentage}%` }}
        />
      </div>

      {/* Stats row */}
      <div className="grid grid-cols-3 gap-3 mb-4">
        <div className="text-center">
          <div className="text-base font-bold text-gray-100 font-mono">
            {status.current_rev}/{status.total_revs}
          </div>
          <div className="text-[10px] text-gray-500 uppercase tracking-wider">Revisions</div>
        </div>
        <div className="text-center">
          <div className="text-base font-bold text-gray-100 font-mono">
            {status.commits_created}
          </div>
          <div className="text-[10px] text-gray-500 uppercase tracking-wider">Commits</div>
        </div>
        <div className="text-center">
          <div className="text-base font-bold text-gray-100 font-mono">
            {status.batches_pushed}
          </div>
          <div className="text-[10px] text-gray-500 uppercase tracking-wider">Batches</div>
        </div>
      </div>

      {/* Mini terminal */}
      {lastLogLines.length > 0 && (
        <div className="bg-gray-950 border border-gray-700 rounded-lg p-3 mb-4">
          <div className="flex items-center space-x-1.5 mb-2">
            <Terminal className="w-3 h-3 text-gray-500" />
            <span className="text-[10px] text-gray-500 uppercase tracking-wider">Log</span>
          </div>
          <div className="space-y-0.5 font-mono text-xs leading-relaxed max-h-[100px] overflow-hidden">
            {lastLogLines.map((line, i) => (
              <div key={i} className="text-gray-400 truncate">{line}</div>
            ))}
          </div>
        </div>
      )}

      {/* Link */}
      <a
        href={linkTo}
        className="flex items-center justify-center space-x-1 text-sm text-blue-400 hover:text-blue-300 transition-colors"
      >
        <span>View Full Import</span>
        <ArrowRight className="w-4 h-4" />
      </a>
    </div>
  );
}
