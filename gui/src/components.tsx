import { useEffect, useRef, useState, type ReactNode } from 'react';
import { ActionIcon, Tooltip } from '@mantine/core';
import { ExternalLink, Megaphone, Mic, MessagesSquare } from 'lucide-react';
import { loadImage, type Link, type Milestone, type MilestoneState } from './api';
import { YoutubeIcon, YoutubeStudioIcon } from './brand-icons';

/** An image behind the API token, fetched once it scrolls near the viewport. */
export function AuthImage({ path, version, alt, className }: { path: string; version: string; alt: string; className?: string }) {
  const holder = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const [url, setUrl] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    const element = holder.current;
    if (!element) return;
    const observer = new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) {
        setVisible(true);
        observer.disconnect();
      }
    }, { rootMargin: '300px' });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    if (!visible) return;
    let current = true;
    void loadImage(path, version).then(loaded => {
      if (!current) return;
      setUrl(loaded);
      setFailed(loaded === null);
    });
    return () => { current = false; };
  }, [visible, path, version]);
  return <div ref={holder} className={`image-frame ${className ?? ''}`}>
    {url && <img src={url} alt={alt} />}
    {failed && <span className="muted">Image could not be read</span>}
  </div>;
}

const ICONS: Partial<Record<Link['kind'], ReactNode>> = {
  youtube_watch: <YoutubeIcon />,
  youtube_channel: <YoutubeIcon />,
  youtube_studio: <YoutubeStudioIcon />,
  youtube_channel_studio: <YoutubeStudioIcon />,
  recording: <Mic size={18} />,
  destination: <MessagesSquare size={18} />,
  post: <ExternalLink size={18} />,
  compose: <Megaphone size={18} />,
};

/** A compact external link showing only its icon; the label is its name and tooltip. */
export function IconLink({ link, size = 'lg' }: { link: Link; size?: 'md' | 'lg' | 'xl' }) {
  return <Tooltip label={link.label} events={{ hover: true, focus: true, touch: false }}>
    <ActionIcon component="a" href={link.url} target="_blank" rel="noopener noreferrer" variant="default" size={size} aria-label={link.label} data-kind={link.kind}>
      {ICONS[link.kind] ?? <ExternalLink size={18} />}
    </ActionIcon>
  </Tooltip>;
}

/** An external link with its icon and label in full. */
export function LabelledLink({ link }: { link: Link }) {
  return <a className="labelled-link" href={link.url} target="_blank" rel="noopener noreferrer" data-kind={link.kind}>
    <span className="labelled-link-icon">{ICONS[link.kind] ?? <ExternalLink size={18} />}</span>
    <span>{link.label}</span>
    <ExternalLink size={14} aria-hidden="true" className="muted" />
  </a>;
}

export const STATE_LABELS: Record<MilestoneState, string> = {
  not_started: 'Not started',
  present: 'Present',
  possibly_stale: 'Possibly stale',
  failing: 'Failing',
  not_applicable: 'Not applicable',
};

const isDone = (milestone: Milestone) => milestone.state === 'present' || milestone.state === 'not_applicable';

export function milestoneSummary(milestones: Milestone[]): string {
  const done = milestones.filter(isDone).length;
  const counts = (['failing', 'possibly_stale'] as const)
    .map(state => [state, milestones.filter(milestone => milestone.state === state).length] as const)
    .filter(([, count]) => count > 0)
    .map(([state, count]) => `${count} ${STATE_LABELS[state].toLowerCase()}`);
  return [`${done} of ${milestones.length} milestones`, ...counts].join(', ');
}

/** One small mark per milestone. Shape and fill differ by state, not only colour. */
export function MilestoneStrip({ milestones }: { milestones: Milestone[] }) {
  return <div className="milestone-strip" role="img" aria-label={milestoneSummary(milestones)}>
    {milestones.map(milestone => <span key={milestone.id} className="mark" data-state={milestone.state}
      title={`${milestone.label}: ${STATE_LABELS[milestone.state]}${milestone.detail ? ` (${milestone.detail})` : ''}`} />)}
  </div>;
}

export function MilestoneList({ milestones }: { milestones: Milestone[] }) {
  return <ol className="milestone-list">
    {milestones.map(milestone => <li key={milestone.id} data-state={milestone.state}>
      <span className="mark" data-state={milestone.state} aria-hidden="true" />
      <span className="milestone-label">{milestone.label}</span>
      <span className="status status-label" data-status={milestone.state}>{STATE_LABELS[milestone.state]}</span>
      {milestone.detail && <span className="milestone-detail muted">{milestone.detail}</span>}
    </li>)}
  </ol>;
}
