import { STATUS_PRIORITY, type Status, type AgentStatus } from './types';

export function aggregate(agents: AgentStatus[]): Status {
  if (agents.length === 0) return 'offline';
  return agents.reduce((best, a) =>
    STATUS_PRIORITY[a.status as Status] < STATUS_PRIORITY[best]
      ? a.status as Status
      : best,
    'offline' as Status
  );
}

export function escHtml(s: string): string {
  return s.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}
