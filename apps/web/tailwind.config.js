/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        // YUKTI identity — cinematic charcoal investigation board, warm ivory type,
        // parchment evidence accents and a single forensic crimson.
        ink: {
          950: '#080807',
          900: '#11100E',
          850: '#15130F',
          800: '#191714',
          700: '#211D19',
          line: '#26221D',
          strong: '#3A342D',
        },
        fg: {
          DEFAULT: '#F1E9DB',
          bright: '#F7F4EE',
          muted: '#A9A197',
          subtle: '#857D72',
        },
        paper: {
          DEFAULT: '#D8C5A7',
          light: '#E9D7B8',
          dark: '#B9A584',
        },
        crimson: {
          DEFAULT: '#E32636',
          deep: '#9F1C26',
          muted: '#702128',
        },
        steel: {
          DEFAULT: '#8FA9C4',
          dim: '#4F6378',
        },
        // Semantic evidence palette — colours carry meaning, never decoration.
        signal: {
          accent: '#D8C5A7', // warm accent · inferred relationships (dashed)
          observed: '#F1E9DB', // directly observed facts (solid)
          supplied: '#8FA9C4', // information supplied by a party or record holder
          flag: '#E32636', // contradictions / requires review
          ok: '#6FBF8A', // verified / healthy (small semantic use only)
        },
      },
      fontFamily: {
        serif: ['Fraunces', 'Georgia', 'Times New Roman', 'serif'],
        display: ['Archivo', 'Inter', 'system-ui', 'sans-serif'],
        sans: ['Inter', 'system-ui', 'Avenir', 'Helvetica', 'Arial', 'sans-serif'],
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
      transitionDuration: {
        fast: '150ms',
        base: '250ms',
        slow: '600ms',
      },
    },
  },
  plugins: [],
}
