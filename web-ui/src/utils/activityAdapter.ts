import type { ActivityEvent } from '@appforgeapps/uiforge';
import type { AuditEntry, SyncRecord } from '../api';

/**
 * Human-readable labels for audit action types.
 */
const ACTION_LABELS: Record<string, string> = {
  sync_cycle: 'Sync Cycle',
  conflict_detected: 'Conflict Detected',
  conflict_resolved: 'Conflict Resolved',
  sync_error: 'Sync Error',
  webhook_received: 'Webhook Received',
  daemon_started: 'Daemon Started',
  auth_login: 'Auth Login',
  config_updated: 'Config Updated',
};

/**
 * Convert an AuditEntry to an ActivityEvent for UIForgeActivityStream.
 *
 * The `type` field encodes both action and success so that UIForge's grouping
 * only groups entries with the same (action, success) pair — matching the
 * existing groupEntries() behavior.
 */
export function auditEntryToActivityEvent(
  entry: AuditEntry,
  repoName?: string,
): ActivityEvent {
  const baseAction = entry.action;
  const type = entry.success ? baseAction : `${baseAction}_failed`;
  const title = ACTION_LABELS[baseAction] ?? baseAction.replace(/_/g, ' ');

  return {
    id: entry.id,
    type,
    title: entry.success ? title : `${title} (Failed)`,
    description: entry.details ?? undefined,
    timestamp: entry.created_at,
    metadata: {
      repository: repoName ?? 'default',
      action: baseAction,
      success: entry.success,
      direction: entry.direction,
      svn_rev: entry.svn_rev,
      git_sha: entry.git_sha,
      author: entry.author,
    },
  };
}

/**
 * Convert a SyncRecord to an ActivityEvent for UIForgeActivityStream.
 *
 * Uses direction as the type so consecutive SVN→Git or Git→SVN syncs
 * are grouped together (e.g., batch imports).
 */
export function syncRecordToActivityEvent(
  record: SyncRecord,
  repoName?: string,
): ActivityEvent {
  const type = `sync_${record.direction}`;
  const truncatedMsg =
    record.message.length > 200
      ? record.message.slice(0, 197) + '...'
      : record.message;

  return {
    id: record.id,
    type,
    title: truncatedMsg || `${record.direction === 'svn_to_git' ? 'SVN \u2192 Git' : 'Git \u2192 SVN'} sync`,
    description: record.message,
    timestamp: record.synced_at,
    metadata: {
      repository: repoName ?? 'default',
      direction: record.direction,
      svn_rev: record.svn_rev,
      git_sha: record.git_sha,
      author: record.author,
      status: record.status,
      committed_at: record.timestamp,
    },
  };
}
