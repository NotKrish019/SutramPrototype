import React from 'react';

export interface PhoneMessage {
  time: string;
  /** Message text. Omit to render a redaction bar (content not relevant / withheld). */
  text?: string;
  outgoing?: boolean;
  highlight?: boolean;
}

const DEFAULT_MESSAGES: PhoneMessage[] = [
  { time: '22:18:40' },
  { time: '22:21:02', outgoing: true },
  { time: '22:29:55' },
  { time: '22:36:14', text: 'at the gate in 5', outgoing: true, highlight: true },
  { time: '22:37:30' },
];

/**
 * A handset holding a communication export (EV-03, device D-01). Messages not
 * relevant to the demonstration are shown redacted rather than invented.
 */
export const PhoneEvidence: React.FC<{ messages?: PhoneMessage[]; className?: string; compact?: boolean }> = ({
  messages = DEFAULT_MESSAGES,
  className = '',
  compact = false,
}) => (
  <div
    className={`relative mx-auto aspect-[9/17.5] rounded-[1.6rem] bg-[#0b0b0c] p-[5px] ${className}`}
    style={{
      boxShadow:
        'inset 0 0 0 1px #3d3a36, inset 0 0 0 2.5px #121212, 0 0 0 1px #000, 5px 7px 0 -1px #1c1a18, 6px 9px 0 -1px #050505, 0 30px 40px -12px rgba(0,0,0,.85)',
    }}
    role="img"
    aria-label="Synthetic communication export on device D-01; one message at 22:36:14 reads “at the gate in 5”; other messages redacted"
  >
    <div className="relative flex h-full flex-col overflow-hidden rounded-[1.25rem] bg-gradient-to-b from-[#16181b] to-[#0d0e10]">
      <div className="mx-auto mt-1.5 h-[9px] w-[34%] rounded-full bg-black" />
      <div className="flex items-center justify-between px-3 pt-1.5 font-mono text-[6.5px] uppercase tracking-[0.1em] text-[#8a8f96]">
        <span>D-01</span>
        <span className="text-crimson">EV-03</span>
      </div>
      <div className="mx-2.5 mt-1.5 border-b border-white/5 pb-1 font-mono text-[6px] uppercase tracking-[0.08em] text-[#6b7078]">
        export · read-only
      </div>
      <ul className={`flex flex-1 flex-col justify-center ${compact ? 'gap-1' : 'gap-1.5'} px-2.5 py-2`}>
        {messages.map((m) => (
          <li key={m.time} className={`flex flex-col ${m.outgoing ? 'items-end' : 'items-start'}`}>
            <span
              className={`max-w-[85%] rounded-md px-1.5 py-1 text-[7.5px] leading-tight ${
                m.highlight
                  ? 'bg-crimson/90 text-white ring-1 ring-crimson/60 ring-offset-1 ring-offset-[#0d0e10]'
                  : m.outgoing
                    ? 'bg-[#2a3038]'
                    : 'bg-[#1f2328]'
              }`}
            >
              {m.text ?? (
                <span aria-hidden="true" className="block space-y-[3px] py-[1px]">
                  <span className="block h-[3px] w-14 rounded-sm bg-[#4d545d]" />
                  <span className="block h-[3px] w-9 rounded-sm bg-[#4d545d]" />
                </span>
              )}
            </span>
            <span className={`mt-[2px] font-mono text-[5.5px] ${m.highlight ? 'text-crimson' : 'text-[#5d636b]'}`}>{m.time}</span>
          </li>
        ))}
      </ul>
      <div className="mx-auto mb-1.5 h-[3px] w-[30%] rounded-full bg-white/20" />
    </div>
  </div>
);
