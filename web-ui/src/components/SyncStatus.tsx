import { type SyncStatus as SyncStatusType } from '../api';

interface Props {
  status: SyncStatusType;
}

export default function SyncStatus({ status }: Props) {
  const stateColor =
    status.state === 'idle'
      ? 'bg-green-400'
      : status.state === 'error'
        ? 'bg-red-400'
        : 'bg-yellow-400';

  const stateLabel =
    status.state.charAt(0).toUpperCase() + status.state.slice(1);

  return (
    <div className="flex items-center space-x-2 text-sm text-gray-300">
      <span className={`inline-block w-2 h-2 rounded-full ${stateColor}`} />
      <span>{stateLabel}</span>
      {status.last_sync_at && (
        <span className="text-gray-500">
          Last sync: {new Date(status.last_sync_at).toLocaleTimeString()}
        </span>
      )}
    </div>
  );
}
