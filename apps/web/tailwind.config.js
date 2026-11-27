/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // YUKTI brand system (2026 redesign): matte black foundation, white type, one forensic
        // crimson for critical intelligence and active states, silver/graphite for structure.
        // Token names are kept from v1 so every component re-themes from one place.
        ink: {
          950: '#0A0A0B',
          900: '#111113',
          850: '#151517',
          800: '#19191C',
          700: '#202024',
          line: '#26262B',
          strong: '#3A3A41',
        },
        fg: {
          DEFAULT: '#EDEDEF',
          bright: '#FFFFFF',
          muted: '#A1A1AA',
          subtle: '#71717A',
        },
        paper: {
          DEFAULT: '#C8CAD0',
          light: '#E4E5E9',
          dark: '#9A9CA4',
        },
        crimson: {
          DEFAULT: '#E5243B',
          deep: '#A3172B',
          muted: '#5C1420',
        },
        steel: {
          DEFAULT: '#8C9EB3',
          dim: '#4F6378',
        },
        // Semantic evidence palette — colours carry meaning, never decoration.
        signal: {
          accent: '#C8CAD0', // silver · inferred relationships (dashed), focus, links
          observed: '#EDEDEF', // directly observed facts (solid)
          supplied: '#8C9EB3', // information supplied by a party or record holder
          flag: '#E5243B', // contradictions / requires review
          ok: '#4FB37D', // verified / healthy (small semantic use only)
          warn: '#D9A441', // assumed / unverified values (never decoration)
          live: '#E5243B', // active state indicator (crimson, used sparingly)
        },
      },
      fontFamily: {
        // Headings use wide Archivo (see .font-serif in workspace.css for the width axis).
        serif: ['Archivo', 'Inter', 'system-ui', 'sans-serif'],
        display: ['Archivo', 'Inter', 'system-ui', 'sans-serif'],
        sans: ['Inter', 'system-ui', 'Avenir', 'Helvetica', 'Arial', 'sans-serif'],
        editorial: ['Fraunces', 'Georgia', 'serif'],
        mono: ['"JetBrains Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'monospace'],
      },
      fontSize: {
        'display-xl': ['clamp(2.75rem, 7.2vw, 6.25rem)', { lineHeight: '0.92', letterSpacing: '-0.025em' }],
        'display-lg': ['clamp(2.25rem, 5vw, 4.25rem)', { lineHeight: '0.95', letterSpacing: '-0.02em' }],
        'display-md': ['clamp(1.75rem, 3.4vw, 2.75rem)', { lineHeight: '1', letterSpacing: '-0.015em' }],
        'editorial-lg': ['clamp(2.25rem, 4.6vw, 4rem)', { lineHeight: '1.02', letterSpacing: '-0.02em' }],
        'editorial-md': ['clamp(1.75rem, 3vw, 2.625rem)', { lineHeight: '1.08', letterSpacing: '-0.015em' }],
        eyebrow: ['0.75rem', { lineHeight: '1rem', letterSpacing: '0.08em' }],
        micro: ['0.6875rem', { lineHeight: '0.875rem', letterSpacing: '0.06em' }],
      },
      maxWidth: {
        site: '80rem',
        wide: '90rem',
      },
      spacing: {
        section: 'clamp(5rem, 11vw, 9rem)',
        gutter: 'clamp(1rem, 4vw, 3.5rem)',
      },
      transitionTimingFunction: {
        out: 'cubic-bezier(0.22, 1, 0.36, 1)',
      },
      boxShadow: {
        lift: '0 1px 0 rgba(255,255,255,0.04) inset, 0 12px 32px -12px rgba(0,0,0,0.7)',
        flag: '0 0 0 1px rgba(229,36,59,0.55), 0 0 24px -6px rgba(229,36,59,0.45)',
      },
      transitionDuration: {
        fast: '150ms',
        base: '250ms',
        slow: '600ms',
      },
    },
  },
  plugins: [],
}
