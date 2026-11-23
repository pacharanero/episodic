import { useEffect, useRef, useState } from 'react';
import { Alert, Button, CopyButton, Group, Modal, ScrollArea, Table, Title } from '@mantine/core';
import { ArrowLeft, Check, Copy, Eye, FolderOpen } from 'lucide-react';
import { framePath, loadPreview, thumbnailPath, type Entry, type Preview, type Publicity } from './api';
import { AuthImage, IconLink, LabelledLink, MilestoneList, milestoneSummary } from './components';

/** Artefact kinds the server can preview as text. */
const PREVIEWABLE = new Set(['metadata', 'show_notes', 'chapters', 'tags', 'description', 'captions']);

const ARTEFACT_LABELS: Record<string, string> = {
  metadata: 'Episode record',
  show_notes: 'Show notes',
  edited_video: 'Edited master',
  upload_video: 'Upload master',
  captions: 'Captions (SRT)',
  chapters: 'Chapters',
  tags: 'Tags',
  description: 'Description',
  thumbnail: 'Thumbnail',
};

export function EpisodeView({ item, back }: { item: Entry; back: () => void }) {
  const report = item.report;
  const heading = useRef<HTMLHeadingElement>(null);
  const [preview, setPreview] = useState<{ kind: string; label: string } | null>(null);
  const [frame, setFrame] = useState<string | null>(null);
  useEffect(() => { heading.current?.focus(); }, [item.entry.path]);
  const base = report?.base ?? item.entry.base;
  const title = report?.fields.title ?? item.entry.base;

  return <article className="episode-view">
    <Button variant="subtle" leftSection={<ArrowLeft size={16} />} onClick={back}>Episodes</Button>
    <div className="episode-heading">
      <div>
        <Title order={2} tabIndex={-1} ref={heading}>{title}</Title>
        <p><span className="stage-label" data-stage={item.entry.stage}>{item.entry.stage}</span> · {item.entry.date}{report?.fields.guest && ` · ${report.fields.guest}`}</p>
      </div>
      {item.links.youtube.length > 0 && <Group gap="xs">{item.links.youtube.map(link => <IconLink key={link.url} link={link} size="xl" />)}</Group>}
    </div>
    <p className="path-row"><FolderOpen size={14} aria-hidden="true" /> <span className="path">{item.entry.path}</span>
      <CopyButton value={item.entry.path}>{({ copied, copy }) =>
        <Button variant="subtle" size="compact-sm" onClick={copy} leftSection={copied ? <Check size={14} /> : <Copy size={14} />}>{copied ? 'Copied' : 'Copy path'}</Button>}
      </CopyButton>
    </p>
    {item.error && <Alert color="yellow">{item.error}</Alert>}

    <div className="episode-layout">
      <div className="episode-main">
        {report && <>
          <section aria-labelledby="next-action">
            <h3 id="next-action">Next action</h3>
            <p>{report.next_action}</p>
            <p>Local upload readiness: <span className="status status-label" data-status={report.ready_for_upload ? 'ready' : 'warning'}>{report.ready_for_upload ? 'ready' : 'not ready'}</span>. This is not a remote verification.</p>
          </section>

          <section aria-labelledby="milestones">
            <h3 id="milestones">Milestones</h3>
            <p className="muted">{milestoneSummary(report.milestones)}. Derived from the files on every read; nothing is stored.</p>
            <MilestoneList milestones={report.milestones} />
          </section>

          <section aria-labelledby="thumbnail">
            <h3 id="thumbnail">Thumbnail</h3>
            {item.thumbnail
              ? <AuthImage className="thumbnail-large" path={thumbnailPath(base)} version={item.thumbnail} alt={`Thumbnail for ${title}`} />
              : <p className="muted">No thumbnail yet. Extract frame candidates with <code>eps thumbnail frames</code>, then compose one with <code>eps thumbnail compose</code>.</p>}
            {item.frames.length > 0 && <>
              <h4>Frame candidates <span className="count">{item.frames.length}</span></h4>
              <ul className="frames">
                {item.frames.map(name => <li key={name}>
                  <button type="button" className="frame" onClick={() => setFrame(name)} aria-label={`Enlarge ${name}`}>
                    <AuthImage path={framePath(base, name)} version={name} alt="" />
                  </button>
                  <span className="muted">{name}</span>
                </li>)}
              </ul>
            </>}
          </section>

          <section aria-labelledby="artefacts">
            <h3 id="artefacts">Artefacts</h3>
            <ScrollArea type="auto"><Table miw={640}><Table.Thead><Table.Tr><Table.Th>Artefact</Table.Th><Table.Th>Status</Table.Th><Table.Th>Bytes</Table.Th><Table.Th>File</Table.Th><Table.Th><span className="visually-hidden">Preview</span></Table.Th></Table.Tr></Table.Thead><Table.Tbody>
              {report.artefacts.map(artefact => {
                const label = ARTEFACT_LABELS[artefact.kind] ?? artefact.kind;
                return <Table.Tr key={artefact.kind}>
                  <Table.Td>{label}</Table.Td>
                  <Table.Td><span className="status status-label" data-status={artefact.exists ? 'present' : 'missing'}>{artefact.exists ? 'Present, not independently verified' : 'Missing'}</span></Table.Td>
                  <Table.Td>{artefact.bytes?.toLocaleString() ?? '-'}</Table.Td>
                  <Table.Td className="path">{artefact.path.split('/').pop()}</Table.Td>
                  <Table.Td>{artefact.exists && PREVIEWABLE.has(artefact.kind) &&
                    <Button variant="default" size="compact-sm" leftSection={<Eye size={14} />} onClick={() => setPreview({ kind: artefact.kind, label })} aria-label={`Preview ${label}`}>Preview</Button>}</Table.Td>
                </Table.Tr>;
              })}
            </Table.Tbody></Table></ScrollArea>
          </section>

          <section aria-labelledby="findings">
            <h3 id="findings">Findings</h3>
            {report.findings.length === 0 ? <p>No findings from local inspection.</p> : <ul>{report.findings.map(finding => <li key={`${finding.code}:${finding.message}`}><strong className="status" data-status={finding.severity}>{finding.severity}: </strong>{finding.message}</li>)}</ul>}
          </section>
        </>}
        <p className="muted">Lifecycle moves, generation, file edits and YouTube publication are not enabled in this read-only increment.</p>
      </div>

      <aside className="tools" aria-labelledby="links-and-tools">
        <h3 id="links-and-tools">Links and tools</h3>
        <LinkGroup title="YouTube" empty="No video ID recorded yet." links={item.links.youtube} />
        {item.links.recording.length > 0 && <LinkGroup title="Recording" links={item.links.recording} />}
        {item.links.destinations.length > 0 && <LinkGroup title="Channel destinations" links={item.links.destinations} />}
        <section aria-labelledby="publicity">
          <h4 id="publicity">Publicity</h4>
          {item.links.publicity.length === 0
            ? <p className="muted">No publicity sections in the episode record.</p>
            : item.links.publicity.map(entry => <PublicityItem key={entry.platform} item={entry} />)}
          <p className="muted">Compose links open the platform's own composer. Nothing is posted from here.</p>
        </section>
      </aside>
    </div>

    <PreviewModal base={base} preview={preview} close={() => setPreview(null)} />
    <Modal opened={frame !== null} onClose={() => setFrame(null)} title={frame ?? ''} size="80rem">
      {frame && <AuthImage className="frame-large" path={framePath(base, frame)} version={frame} alt={`Frame candidate ${frame}`} />}
    </Modal>
  </article>;
}

function LinkGroup({ title, links, empty }: { title: string; links: Entry['links']['youtube']; empty?: string }) {
  return <section className="link-group">
    <h4>{title}</h4>
    {links.length === 0 ? <p className="muted">{empty}</p> : <ul>{links.map(link => <li key={link.url}><LabelledLink link={link} /></li>)}</ul>}
  </section>;
}

function PublicityItem({ item }: { item: Publicity }) {
  const over = item.max_characters !== null && item.characters > item.max_characters;
  return <div className="publicity" data-platform={item.platform.toLowerCase()}>
    <div className="publicity-heading">
      <strong>{item.platform}</strong>
      {item.post_url
        ? <span className="status status-label" data-status="ready">Posted{item.post_date && ` ${item.post_date}`}</span>
        : <span className="status status-label" data-status="missing">Not posted</span>}
    </div>
    {item.text
      ? <>
        <p className="publicity-text">{item.text}</p>
        <p className="muted">
          <span className={over ? 'status' : undefined} data-status={over ? 'error' : undefined}>
            {item.characters}{item.max_characters !== null && ` / ${item.max_characters}`} characters
          </span>
          {item.text_source === 'title_and_link' && ' · no draft in the record; title and watch link'}
        </p>
        <Group gap="xs">
          <CopyButton value={item.text}>{({ copied, copy }) =>
            <Button variant="default" size="compact-sm" onClick={copy} leftSection={copied ? <Check size={14} /> : <Copy size={14} />}>{copied ? 'Copied' : 'Copy text'}</Button>}
          </CopyButton>
          {item.compose && <Button component="a" href={item.compose.url} target="_blank" rel="noopener noreferrer" variant="default" size="compact-sm">{item.compose.label}</Button>}
          {item.post_url && <Button component="a" href={item.post_url.url} target="_blank" rel="noopener noreferrer" variant="subtle" size="compact-sm">View post</Button>}
        </Group>
      </>
      : <p className="muted">No draft yet, and no video ID to share.</p>}
  </div>;
}

function PreviewModal({ base, preview, close }: { base: string; preview: { kind: string; label: string } | null; close: () => void }) {
  const [state, setState] = useState<{ data?: Preview; error?: string }>({});
  useEffect(() => {
    if (!preview) return;
    const controller = new AbortController();
    setState({});
    loadPreview(base, preview.kind, controller.signal)
      .then(data => setState({ data }))
      .catch(failure => { if (!controller.signal.aborted) setState({ error: failure instanceof Error ? failure.message : 'Preview failed.' }); });
    return () => controller.abort();
  }, [base, preview]);
  return <Modal opened={preview !== null} onClose={close} title={preview?.label ?? ''} size="xl">
    {!state.data && !state.error && <p role="status">Reading file…</p>}
    {state.error && <Alert color="red" role="alert">{state.error}</Alert>}
    {state.data && <>
      <Group justify="space-between" mb="sm">
        <span className="muted">{state.data.bytes.toLocaleString()} bytes{state.data.truncated && ' · showing the first 256 KB'}</span>
        <CopyButton value={state.data.text}>{({ copied, copy }) =>
          <Button variant="default" size="compact-sm" onClick={copy} leftSection={copied ? <Check size={14} /> : <Copy size={14} />}>{copied ? 'Copied' : 'Copy'}</Button>}
        </CopyButton>
      </Group>
      <pre className="preview">{state.data.text}</pre>
    </>}
  </Modal>;
}
