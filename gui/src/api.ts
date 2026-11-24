export type MilestoneState = 'not_started' | 'present' | 'possibly_stale' | 'failing' | 'not_applicable';

export interface Milestone {
  id: string;
  label: string;
  state: MilestoneState;
  detail: string | null;
}

export interface Report {
  base: string;
  stage: string;
  episode_path: string;
  fields: { title: string | null; guest: string | null; episode_date: string | null; privacy_status: string | null; video_id: string | null };
  artefacts: { kind: string; path: string; exists: boolean; bytes: number | null }[];
  ready_for_upload: boolean;
  next_action: string;
  findings: { severity: string; code: string; message: string }[];
  milestones: Milestone[];
}

export type LinkKind = 'youtube_watch' | 'youtube_studio' | 'youtube_channel' | 'youtube_channel_studio' | 'recording' | 'destination' | 'post' | 'compose';

export interface Link {
  kind: LinkKind;
  label: string;
  url: string;
}

export interface Publicity {
  platform: string;
  text: string | null;
  text_source: 'draft' | 'title_and_link' | null;
  characters: number;
  max_characters: number | null;
  post_url: Link | null;
  post_date: string | null;
  compose: Link | null;
}

export interface Entry {
  entry: { base: string; path: string; stage: string; date: string | null };
  report: Report | null;
  error: string | null;
  links: { youtube: Link[]; recording: Link[]; destinations: Link[]; publicity: Publicity[] };
  frames: string[];
  thumbnail: string | null;
}

/** Must match `API_VERSION` in src/gui.rs. */
export const API_VERSION = 2;

export interface Workspace {
  api_version: number;
  channel: { name: string; id: string };
  channel_links: Link[];
  stages: string[];
  models: { task: string; profile: string; provider: string; model: string }[];
  episodes: Entry[];
  fingerprint: string;
}

export interface Preview {
  kind: string;
  bytes: number;
  truncated: boolean;
  text: string;
}

const fragment = location.hash.slice(1);
if (fragment) {
  sessionStorage.setItem('eps-token', fragment);
  history.replaceState(null, '', location.pathname);
}

async function authorised(path: string, signal?: AbortSignal): Promise<Response> {
  const token = sessionStorage.getItem('eps-token');
  if (!token) throw new Error('Open the private URL printed by eps gui in this browser tab.');
  const response = await fetch(path, { headers: { Authorization: `Bearer ${token}` }, cache: 'no-store', signal });
  if (response.status === 403) throw new Error('Access refused. Reopen the URL printed by the running eps gui server.');
  return response;
}

export async function loadWorkspace(signal: AbortSignal): Promise<Workspace> {
  const response = await authorised('/api/workspace', signal);
  if (!response.ok) throw new Error('Could not read workspace. Check the server and configuration, then refresh.');
  const data = (await response.json()) as Partial<Workspace>;
  if (data.api_version !== API_VERSION) {
    throw new Error('This page and the running eps gui server come from different builds. Reload the page; if that does not help, stop the server and start it again with s/gui-dev.');
  }
  return data as Workspace;
}

/** A cheap marker that changes when episode files change; null when unavailable. */
export async function loadFingerprint(): Promise<string | null> {
  try {
    const response = await authorised('/api/fingerprint');
    if (!response.ok) return null;
    return ((await response.json()) as { fingerprint: string }).fingerprint;
  } catch {
    return null;
  }
}

export async function loadPreview(base: string, kind: string, signal: AbortSignal): Promise<Preview> {
  const response = await authorised(`/api/episodes/${encodeURIComponent(base)}/artefacts/${encodeURIComponent(kind)}`, signal);
  if (!response.ok) throw new Error('This file could not be read. It may have moved since the last refresh.');
  return response.json() as Promise<Preview>;
}

export const thumbnailPath = (base: string) => `/api/episodes/${encodeURIComponent(base)}/thumbnail`;
export const framePath = (base: string, name: string) => `/api/episodes/${encodeURIComponent(base)}/frames/${encodeURIComponent(name)}`;

// Images need the bearer token, so they are fetched and shown from blob URLs.
// One object URL per path is kept; a new version replaces and revokes it.
const images = new Map<string, { version: string; url: Promise<string | null> }>();

export function loadImage(path: string, version: string): Promise<string | null> {
  const cached = images.get(path);
  if (cached?.version === version) return cached.url;
  if (cached) void cached.url.then(url => url && URL.revokeObjectURL(url));
  const url = authorised(path)
    .then(async response => (response.ok ? URL.createObjectURL(await response.blob()) : null))
    .catch(() => null);
  images.set(path, { version, url });
  return url;
}
