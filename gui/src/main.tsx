import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Alert, Badge, Button, Group, MantineProvider, Modal, ScrollArea, Table, TextInput, Title } from '@mantine/core';
import { RefreshCw, Settings, Search } from 'lucide-react';
import '@mantine/core/styles.css';
import './style.css';
import { theme } from './theme';
import { loadFingerprint, loadWorkspace, thumbnailPath, type Entry, type Workspace } from './api';
import { AuthImage, IconLink, MilestoneStrip, milestoneSummary } from './components';
import { EpisodeView } from './episode';

/** How often the page asks whether any episode file changed. */
const WATCH_INTERVAL_MS = 5000;

function App() {
  const [workspace, setWorkspace] = useState<Workspace | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [updated, setUpdated] = useState<Date | null>(null);
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState<string | null>(null);
  const [showChannel, setShowChannel] = useState(false);
  const request = useRef<AbortController | null>(null);
  const fingerprint = useRef<string | null>(null);
  const refresh = useCallback(async () => {
    request.current?.abort();
    const controller = new AbortController();
    request.current = controller;
    setBusy(true);
    try {
      const data = await loadWorkspace(controller.signal);
      fingerprint.current = data.fingerprint;
      setWorkspace(data);
      setUpdated(new Date());
      setError('');
    } catch (failure) {
      if (!controller.signal.aborted) setError(failure instanceof Error ? failure.message : 'Workspace request failed.');
    } finally {
      if (!controller.signal.aborted) setBusy(false);
    }
  }, []);
  useEffect(() => { void refresh(); return () => request.current?.abort(); }, [refresh]);

  // Files change outside the browser (eps runs, editors, sync). Poll a cheap
  // marker while the tab is visible and reload only when it moves.
  useEffect(() => {
    const timer = window.setInterval(() => {
      if (document.visibilityState !== 'visible' || fingerprint.current === null) return;
      void loadFingerprint().then(current => {
        if (current && current !== fingerprint.current) void refresh();
      });
    }, WATCH_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [refresh]);

  const episode = workspace?.episodes.find(item => item.entry.path === selected);
  return <>
    <a className="skip" href="#main">Skip to episodes</a>
    <header>
      <div className="identity">
        <div><span className="brand">episodic</span><Title order={1} size="h3">{workspace?.channel.name ?? 'Channel workbench'}</Title></div>
        {workspace && workspace.channel_links.length > 0 && <Group gap="xs">{workspace.channel_links.map(link => <IconLink key={link.url} link={link} />)}</Group>}
      </div>
      <Group>
        <Badge color="gray" variant="outline">Read-only preview</Badge>
        <Button variant="default" leftSection={<Settings size={16} />} disabled={!workspace} onClick={() => setShowChannel(true)}>Channel</Button>
        <Button variant="default" leftSection={<RefreshCw size={16} />} loading={busy} onClick={() => void refresh()}>Refresh</Button>
      </Group>
    </header>
    <main id="main">
      {error && <Alert color="red" title="Workspace unavailable" role="alert">{error}{workspace && ' Displayed data is from the last successful refresh.'}</Alert>}
      {!workspace && busy && <p role="status">Reading episode folders…</p>}
      {workspace && !episode && <>
        <div className="toolbar"><Title order={2}>Episodes</Title><TextInput label="Find an episode" placeholder="Title, guest or date" leftSection={<Search size={16} />} value={query} onChange={event => setQuery(event.currentTarget.value)} /></div>
        {workspace.episodes.length === 0 && <p>No episode folders found. Create one with <code>eps episode new</code>, then refresh.</p>}
        {workspace.episodes.length > 0 && <div className="board">{workspace.stages.map(stage => {
          const entries = workspace.episodes.filter(item => item.entry.stage === stage && `${item.entry.base} ${item.report?.fields.title ?? ''} ${item.report?.fields.guest ?? ''}`.toLowerCase().includes(query.toLowerCase()));
          return <section className="stage" data-stage={stage} key={stage} aria-label={stage}>
            <h3>{stage} <span className="count">{entries.length}</span></h3>
            {entries.length === 0 && <p className="muted">{query ? 'No matching episodes' : 'No episodes'}</p>}
            {entries.map(item => <EpisodeCard key={item.entry.path} item={item} open={() => setSelected(item.entry.path)} />)}
          </section>;
        })}</div>}
      </>}
      {episode && <EpisodeView item={episode} back={() => setSelected(null)} />}
      {selected && workspace && !episode && <p role="status">That episode has moved or disappeared. <Button variant="subtle" onClick={() => setSelected(null)}>Return to episodes</Button></p>}
    </main>
    <footer>
      <span>Local files are the source of truth. Changes made outside the browser appear within a few seconds. No remote services are contacted by this view.</span>
      {updated && <span role="status" aria-live="polite">Updated {updated.toLocaleTimeString()}</span>}
    </footer>
    <Modal opened={showChannel} onClose={() => setShowChannel(false)} title="Channel model routing" size="xl">
      <p>Configured models, not verified availability. Routing changes remain an eps.toml edit.</p>
      <ScrollArea><Table><Table.Thead><Table.Tr><Table.Th>Task</Table.Th><Table.Th>Profile</Table.Th><Table.Th>Provider</Table.Th><Table.Th>Model</Table.Th></Table.Tr></Table.Thead><Table.Tbody>
        {workspace?.models.map(route => <Table.Tr key={route.task}><Table.Td>{route.task}</Table.Td><Table.Td>{route.profile}</Table.Td><Table.Td>{route.provider}</Table.Td><Table.Td>{route.model}</Table.Td></Table.Tr>)}
      </Table.Tbody></Table></ScrollArea>
      {workspace?.models.length === 0 && <p>No model profiles configured.</p>}
    </Modal>
  </>;
}

/**
 * One episode on the board. The title is the button that opens the episode;
 * its hit area covers the card, while the YouTube links stay separate
 * controls rather than being nested inside another interactive element.
 */
function EpisodeCard({ item, open }: { item: Entry; open: () => void }) {
  const report = item.report;
  const title = report?.fields.title ?? item.entry.base;
  return <article className="episode" data-stage={item.entry.stage}>
    {item.thumbnail && <AuthImage className="card-thumbnail" path={thumbnailPath(item.entry.base)} version={item.thumbnail} alt="" />}
    <div className="card-body">
      <span className="muted">{item.entry.date}</span>
      <h4 className="card-title"><button type="button" className="card-open" onClick={open}>{title}</button></h4>
      {report?.fields.guest && <span>{report.fields.guest}</span>}
      {report && <MilestoneStrip milestones={report.milestones} />}
      <span className={`next${item.error ? ' status' : ''}`} data-status={item.error ? 'warning' : undefined}>{report?.next_action ?? item.error}</span>
      <div className="card-footer">
        {report && <span className="muted">{milestoneSummary(report.milestones)}</span>}
        {item.links.youtube.length > 0 && <Group gap={6} className="card-actions">{item.links.youtube.map(link => <IconLink key={link.url} link={link} size="lg" />)}</Group>}
      </div>
    </div>
  </article>;
}

createRoot(document.getElementById('root')!).render(<React.StrictMode><MantineProvider theme={theme}><App /></MantineProvider></React.StrictMode>);
