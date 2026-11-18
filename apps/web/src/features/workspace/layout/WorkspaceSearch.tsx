import React, { useEffect, useId, useMemo, useRef, useState } from 'react';
import { Archive, FolderOpen, ListOrdered, Search, Share2, type LucideIcon } from 'lucide-react';
import { caseHref, navigate } from '../../../router/router';
import { useCaseDetail } from '../data/WorkspaceDataContext';
import { useWorkspaceShell } from './WorkspaceShellContext';

interface SearchResult {
  id: string;
  group: 'Cases' | 'Evidence' | 'Entities' | 'Events';
  label: string;
  detail: string;
  href: string;
  icon: LucideIcon;
}

const MAX_PER_GROUP = 5;

function matches(query: string, ...fields: (string | undefined)[]): boolean {
  return fields.some((field) => field?.toLowerCase().includes(query));
}

/** Searches the case list and the active case's records currently loaded in the workspace. */
export const WorkspaceSearch: React.FC = () => {
  const { cases, activeCaseId } = useWorkspaceShell();
  const detail = useCaseDetail(activeCaseId ?? '');
  const [query, setQuery] = useState('');
  const [open, setOpen] = useState(false);
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const listId = useId();

  const results = useMemo<SearchResult[]>(() => {
    const q = query.trim().toLowerCase();
    if (!q) return [];
    const out: SearchResult[] = [];
    if (cases.state.status === 'success') {
      out.push(
        ...cases.state.data
          .filter((c) => matches(q, c.case_id, c.title, c.fir_number))
          .slice(0, MAX_PER_GROUP)
          .map((c) => ({ id: c.case_id, group: 'Cases' as const, label: c.case_id, detail: c.title, href: caseHref(c.case_id), icon: FolderOpen })),
      );
    }
    const data = detail.state.status === 'success' ? detail.state.data : null;
    if (data) {
      const caseId = data.case.case_id;
      out.push(
        ...data.evidence
          .filter((e) => matches(q, e.evidence_id, e.original_filename, e.source, e.source_type ?? '', e.extracted_summary))
          .slice(0, MAX_PER_GROUP)
          .map((e) => ({ id: e.evidence_id, group: 'Evidence' as const, label: e.evidence_id, detail: e.original_filename, href: caseHref(caseId, 'evidence', { evidence: e.evidence_id }), icon: Archive })),
        ...data.entities
          .filter((n) => matches(q, n.entity_id, n.canonical_name, n.type_label, ...n.aliases))
          .slice(0, MAX_PER_GROUP)
          .map((n) => ({ id: n.entity_id, group: 'Entities' as const, label: n.canonical_name, detail: n.type_label, href: caseHref(caseId, 'graph', { node: n.entity_id }), icon: Share2 })),
        ...data.events
          .filter((ev) => matches(q, ev.event_id, ev.title, ev.event_type, ev.location_name))
          .slice(0, MAX_PER_GROUP)
          .map((ev) => ({ id: ev.event_id, group: 'Events' as const, label: ev.title, detail: ev.event_id, href: caseHref(caseId, 'timeline', { event: ev.event_id }), icon: ListOrdered })),
      );
    }
    return out;
  }, [query, cases.state, detail.state]);

  useEffect(() => setCursor(0), [query]);

  // "/" focuses search unless the user is typing somewhere else.
  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      const target = event.target as HTMLElement | null;
      const typing = target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT' || target.isContentEditable);
      if (event.key === '/' && !typing && !event.metaKey && !event.ctrlKey) {
        event.preventDefault();
        inputRef.current?.focus();
      }
    };
    const onPointer = (event: PointerEvent): void => {
      if (rootRef.current && !rootRef.current.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('keydown', onKey);
    document.addEventListener('pointerdown', onPointer);
    return () => {
      document.removeEventListener('keydown', onKey);
      document.removeEventListener('pointerdown', onPointer);
    };
  }, []);

  const choose = (result: SearchResult): void => {
    navigate(result.href);
    setOpen(false);
    setQuery('');
    inputRef.current?.blur();
  };

  const onKeyDown = (event: React.KeyboardEvent<HTMLInputElement>): void => {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      setOpen(true);
      setCursor((value) => Math.min(value + 1, Math.max(results.length - 1, 0)));
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      setCursor((value) => Math.max(value - 1, 0));
    } else if (event.key === 'Enter' && results[cursor]) {
      event.preventDefault();
      choose(results[cursor]!);
    } else if (event.key === 'Escape') {
      setOpen(false);
      setQuery('');
    }
  };

  const showPanel = open && query.trim().length > 0;
  let lastGroup = '';

  return (
    <div ref={rootRef} className="relative w-full max-w-md">
      <label className="relative flex h-9 items-center">
        <span className="sr-only">Search the workspace</span>
        <Search aria-hidden="true" className="pointer-events-none absolute left-3 h-3.5 w-3.5 text-fg-subtle" />
        <input
          ref={inputRef}
          type="text"
          role="combobox"
          aria-expanded={showPanel}
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={showPanel && results[cursor] ? `${listId}-${cursor}` : undefined}
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setOpen(true);
          }}
          onFocus={() => setOpen(true)}
          onKeyDown={onKeyDown}
          placeholder="Search cases, evidence, entities, events"
          className="h-full w-full border border-ink-line bg-ink-950 pl-9 pr-10 text-sm text-fg placeholder:text-fg-subtle focus:border-signal-amber/60 focus:outline-none"
        />
        <kbd className="pointer-events-none absolute right-2.5 hidden border border-ink-strong px-1.5 font-mono text-micro text-fg-subtle sm:block">/</kbd>
      </label>
      {showPanel && (
        <div className="absolute left-0 right-0 top-11 z-50 max-h-[60vh] overflow-y-auto border border-ink-strong bg-ink-900 shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)]">
          {results.length === 0 ? (
            <p className="px-4 py-4 text-sm text-fg-muted">No matches for “{query.trim()}”.</p>
          ) : (
            <ul id={listId} role="listbox" aria-label="Search results">
              {results.map((result, index) => {
                const header = result.group !== lastGroup ? result.group : null;
                lastGroup = result.group;
                return (
                  <li key={`${result.group}-${result.id}`} role="presentation">
                    {header && (
                      <p role="presentation" className="border-t border-ink-line px-4 pb-1 pt-3 font-mono text-micro uppercase text-fg-subtle first:border-t-0">
                        {header}
                      </p>
                    )}
                    <div
                      id={`${listId}-${index}`}
                      role="option"
                      aria-selected={index === cursor}
                      onPointerDown={(event) => event.preventDefault()}
                      onClick={() => choose(result)}
                      onMouseEnter={() => setCursor(index)}
                      className={`flex cursor-pointer items-center gap-3 px-4 py-2 ${index === cursor ? 'bg-white/[0.05]' : ''}`}
                    >
                      <result.icon aria-hidden="true" className="h-3.5 w-3.5 shrink-0 text-fg-subtle" />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-sm text-fg">{result.label}</span>
                        <span className="block truncate font-mono text-micro text-fg-subtle">{result.detail}</span>
                      </span>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
          {detail.state.status === 'success' && detail.state.data && (
            <p className="border-t border-ink-line px-4 py-2 font-mono text-micro uppercase text-fg-subtle">
              Records searched: case list + {activeCaseId}
            </p>
          )}
        </div>
      )}
    </div>
  );
};
