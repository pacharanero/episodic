export interface Report {
  base: string;
  stage: string;
  episode_path: string;
  fields: { title: string | null; guest: string | null; episode_date: string | null; privacy_status: string | null };
  artefacts: { kind: string; path: string; exists: boolean; bytes: number | null }[];
  ready_for_upload: boolean;
  next_action: string;
  findings: { severity: string; code: string; message: string }[];
}

export interface Entry {
  entry: { base: string; path: string; stage: string; date: string | null };
  report: Report | null;
  error: string | null;
  links: { label: string; url: string }[];
}

export interface Workspace {
  channel: { name: string; id: string };
  stages: string[];
  models: { task: string; profile: string; provider: string; model: string }[];
  episodes: Entry[];
}

const fragment = location.hash.slice(1);
if (fragment) {
  sessionStorage.setItem('eps-token', fragment);
  history.replaceState(null, '', location.pathname);
}

export async function loadWorkspace(signal: AbortSignal): Promise<Workspace> {
  const token = sessionStorage.getItem('eps-token');
  if (!token) throw new Error('Open the private URL printed by eps gui in this browser tab.');
  const response = await fetch('/api/workspace', {
    headers: { Authorization: `Bearer ${token}` },
    cache: 'no-store', signal,
  });
  if (!response.ok) {
    if (response.status === 403) throw new Error('Access refused. Reopen the URL printed by the running eps gui server.');
    throw new Error('Could not read workspace. Check the server and configuration, then refresh.');
  }
  return response.json() as Promise<Workspace>;
}
