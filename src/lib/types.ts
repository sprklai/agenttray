export type Status = 'needs-input' | 'error' | 'working' | 'starting' | 'idle' | 'offline';

export interface TerminalInfo {
  kind: string;
  focus_id: string;
  outer_id: string;
  label: string;
}

export interface AgentStatus {
  name: string;
  status: Status;
  message: string;
  terminal: TerminalInfo | null;
  can_focus: boolean;
}

export const STATUS_PRIORITY: Record<Status, number> = {
  'needs-input': 0,
  'error':       1,
  'working':     2,
  'starting':    3,
  'idle':        4,
  'offline':     5,
};

export const STATUS_LABEL: Record<Status, string> = {
  'needs-input': 'needs input',
  'error':       'error',
  'working':     'working',
  'starting':    'starting',
  'idle':        'idle',
  'offline':     'offline',
};

export const STATUS_COLOR: Record<Status, string> = {
  'needs-input': '#dd4f4f',
  'error':       '#cc7a28',
  'working':     '#c99626',
  'starting':    '#4898cc',
  'idle':        '#78b644',
  'offline':     '#555555',
};
