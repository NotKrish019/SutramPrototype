import React, { useEffect, useState } from 'react';
import { Check, Copy } from 'lucide-react';

/** Copy text to the clipboard; resolves false when the browser refuses (insecure context, denied). */
export async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // Fall through to the legacy path below.
  }
  try {
    const area = document.createElement('textarea');
    area.value = text;
    area.setAttribute('readonly', '');
    area.style.position = 'fixed';
    area.style.opacity = '0';
    document.body.appendChild(area);
    area.select();
    const ok = document.execCommand?.('copy') ?? false;
    area.remove();
    return ok;
  } catch {
    return false;
  }
}

/**
 * One `yukti` command rendered as a terminal line with a copy button. The web and the CLI read the
 * same API, so the command reproduces exactly what the view shows.
 */
export const CliCommand: React.FC<{ command: string; note?: string; compact?: boolean; className?: string }> = ({
  command,
  note,
  compact = false,
  className = '',
}) => {
  const [state, setState] = useState<'idle' | 'copied' | 'failed'>('idle');
  useEffect(() => {
    if (state === 'idle') return;
    const timer = setTimeout(() => setState('idle'), 1600);
    return () => clearTimeout(timer);
  }, [state]);

  return (
    <div className={`group relative border border-ink-line bg-black/60 ${className}`}>
      <div className={`flex items-center gap-2 ${compact ? 'px-2.5 py-1.5' : 'px-3 py-2.5'}`}>
        <span aria-hidden="true" className="select-none font-mono text-xs text-crimson">
          $
        </span>
        <code className={`min-w-0 flex-1 overflow-x-auto whitespace-nowrap font-mono text-fg ${compact ? 'text-[0.72rem]' : 'text-xs'}`}>
          {command}
        </code>
        <button
          type="button"
          onClick={() => {
            void copyText(command).then((ok) => setState(ok ? 'copied' : 'failed'));
          }}
          aria-label={`Copy command: ${command}`}
          className="inline-flex h-7 shrink-0 items-center gap-1.5 border border-ink-strong px-2 font-mono text-[0.65rem] uppercase tracking-wider text-fg-muted transition-colors duration-fast hover:border-fg-subtle hover:text-fg focus:outline-none focus-visible:ring-1 focus-visible:ring-signal-accent"
        >
          {state === 'copied' ? <Check aria-hidden="true" className="h-3 w-3 text-signal-ok" /> : <Copy aria-hidden="true" className="h-3 w-3" />}
          <span aria-live="polite">{state === 'copied' ? 'Copied' : state === 'failed' ? 'Select' : 'Copy'}</span>
        </button>
      </div>
      {note && <p className="border-t border-ink-line px-3 py-1.5 font-mono text-[0.65rem] uppercase tracking-wider text-fg-subtle">{note}</p>}
    </div>
  );
};
