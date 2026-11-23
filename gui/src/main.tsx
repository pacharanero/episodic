import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Alert, Badge, Button, Group, MantineProvider, Modal, ScrollArea, Table, TextInput, Title } from '@mantine/core';
import { RefreshCw, Settings, ArrowLeft, Search } from 'lucide-react';
import '@mantine/core/styles.css';
import './style.css';
import { theme } from './theme';
import { loadWorkspace, type Entry, type Workspace } from './api';

function App() {
  const [workspace, setWorkspace] = useState<Workspace | null>(null);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState<string | null>(null);
  const [showChannel, setShowChannel] = useState(false);
  const request = useRef<AbortController | null>(null);
  const refresh = useCallback(async () => {
    request.current?.abort();
    const controller = new AbortController();
    request.current = controller;
    setBusy(true);
    try {
      const data = await loadWorkspace(controller.signal);
      setWorkspace(data);
      setError('');
    } catch (failure) {
      if (!controller.signal.aborted) setError(failure instanceof Error ? failure.message : 'Workspace request failed.');
    } finally {
      if (!controller.signal.aborted) setBusy(false);
    }
  }, []);
  useEffect(() => { void refresh(); return () => request.current?.abort(); }, [refresh]);
  const episode = workspace?.episodes.find(item => item.entry.path === selected);
  return <>
    <a className="skip" href="#main">Skip to episodes</a>
    <header>
      <div><span className="brand">episodic</span><Title order={1} size="h3">{workspace?.channel.name ?? 'Channel workbench'}</Title></div>
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
          return <section className="stage" key={stage} aria-label={stage}>
            <h3>{stage} <span className="count">{entries.length}</span></h3>
            {entries.length === 0 && <p className="muted">{query ? 'No matching episodes' : 'No episodes'}</p>}
            {entries.map(item => <button className="episode" key={item.entry.path} onClick={() => setSelected(item.entry.path)}>
              <span className="muted">{item.entry.date}</span>
              <strong>{item.report?.fields.title ?? item.entry.base}</strong>
              {item.report?.fields.guest && <span>{item.report.fields.guest}</span>}
              <span className="next">{item.report?.next_action ?? item.error}</span>
              {item.report && <span className="muted">{item.report.artefacts.filter(a => a.exists).length}/{item.report.artefacts.length} artefacts present</span>}
            </button>)}
          </section>;
        })}</div>}
      </>}
      {episode && <Episode item={episode} back={() => setSelected(null)} />}
      {selected && workspace && !episode && <p role="status">That episode has moved or disappeared. <Button variant="subtle" onClick={() => setSelected(null)}>Return to episodes</Button></p>}
    </main>
    <footer>Local files are the source of truth. Refresh after external changes. No remote services are contacted by this view.</footer>
    <Modal opened={showChannel} onClose={() => setShowChannel(false)} title="Channel model routing" size="xl">
      <p>Configured models, not verified availability. Routing changes remain an eps.toml edit.</p>
      <ScrollArea><Table><Table.Thead><Table.Tr><Table.Th>Task</Table.Th><Table.Th>Profile</Table.Th><Table.Th>Provider</Table.Th><Table.Th>Model</Table.Th></Table.Tr></Table.Thead><Table.Tbody>
        {workspace?.models.map(route => <Table.Tr key={route.task}><Table.Td>{route.task}</Table.Td><Table.Td>{route.profile}</Table.Td><Table.Td>{route.provider}</Table.Td><Table.Td>{route.model}</Table.Td></Table.Tr>)}
      </Table.Tbody></Table></ScrollArea>
      {workspace?.models.length === 0 && <p>No model profiles configured.</p>}
    </Modal>
  </>;
}

function Episode({ item, back }: { item: Entry; back: () => void }) {
  const report = item.report;
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => { heading.current?.focus(); }, [item.entry.path]);
  return <article>
    <Button variant="subtle" leftSection={<ArrowLeft size={16} />} onClick={back}>Episodes</Button>
    <Title order={2} tabIndex={-1} ref={heading}>{report?.fields.title ?? item.entry.base}</Title>
    <p>{item.entry.stage} · {item.entry.date}{report?.fields.guest && ` · ${report.fields.guest}`}</p>
    <p className="path">{item.entry.path}</p>
    {item.links.length > 0 && <Group>{item.links.map(link => <Button component="a" key={link.url} href={link.url} target="_blank" rel="noopener noreferrer" variant="default">{link.label}</Button>)}</Group>}
    {item.error && <Alert color="yellow">{item.error}</Alert>}
    {report && <>
      <h3>Next action</h3><p>{report.next_action}</p>
      <p>Local upload readiness: {report.ready_for_upload ? 'ready' : 'not ready'}. This is not a remote verification.</p>
      <h3>Artefacts</h3>
      <ScrollArea><Table><Table.Thead><Table.Tr><Table.Th>Artefact</Table.Th><Table.Th>Status</Table.Th><Table.Th>Bytes</Table.Th><Table.Th>Path</Table.Th></Table.Tr></Table.Thead><Table.Tbody>
        {report.artefacts.map(artefact => <Table.Tr key={artefact.kind}><Table.Td>{artefact.kind}</Table.Td><Table.Td>{artefact.exists ? 'Present, not independently verified' : 'Missing'}</Table.Td><Table.Td>{artefact.bytes?.toLocaleString() ?? '-'}</Table.Td><Table.Td className="path">{artefact.path}</Table.Td></Table.Tr>)}
      </Table.Tbody></Table></ScrollArea>
      <h3>Findings</h3>
      {report.findings.length === 0 ? <p>No findings from local inspection.</p> : <ul>{report.findings.map(finding => <li key={`${finding.code}:${finding.message}`}><strong>{finding.severity}: </strong>{finding.message}</li>)}</ul>}
    </>}
    <p className="muted">Lifecycle moves, generation, file edits and YouTube publication are not enabled in this read-only increment.</p>
  </article>;
}

createRoot(document.getElementById('root')!).render(<React.StrictMode><MantineProvider theme={theme}><App /></MantineProvider></React.StrictMode>);
