import { STATUS_PRIORITY, type Status, type AgentStatus } from './types';

export function aggregate(agents: AgentStatus[]): Status {
  if (agents.length === 0) return 'offline';
  return agents.reduce<Status>((best, a) =>
    STATUS_PRIORITY[a.status] < STATUS_PRIORITY[best]
      ? a.status
      : best,
    'offline'
  );
}
