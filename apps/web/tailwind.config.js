/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        brand: {
          50: '#f0f9ff',
          100: '#e0f2fe',
          500: '#0ea5e9',
          600: '#0284c7',
          700: '#0369a1',
          900: '#0c4a6e',
        },
        // YUKTI site design tokens — near-black canvas with hairline structure.
        ink: {
          950: '#09090A',
          900: '#0F0F11',
          850: '#141416',
          800: '#1A1A1D',
          700: '#232327',
          line: '#242428',
          strong: '#36363C',
        },
        fg: {
          DEFAULT: '#EDEDEF',
          muted: '#A3A3AB',
          subtle: '#6E6E77',
        },
        // Semantic evidence palette — colours carry meaning, never decoration.
        signal: {
          amber: '#FFB224', // brand accent · inferred relationships (dashed)
          observed: '#EDEDEF', // directly observed facts (solid)
          supplied: '#7DB4FF', // information supplied by a party or record holder
          flag: '#FF5A4E', // contradictions / requires review
          ok: '#4ADE80', // verified / healthy
        },
      },
      fontFamily: {
        display: ['Archivo', 'Inter', 'system-ui', 'sans-serif'],
        sans: ['Inter', 'system-ui', 'Avenir', 'Helvetica', 'Arial', 'sans-serif'],
        mono: ['"JetBrains Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'monospace'],
      },
      fontSize: {
        'display-xl': ['clamp(2.75rem, 7.2vw, 6.25rem)', { lineHeight: '0.92', letterSpacing: '-0.025em' }],
        'display-lg': ['clamp(2.25rem, 5vw, 4.25rem)', { lineHeight: '0.95', letterSpacing: '-0.02em' }],
        'display-md': ['clamp(1.75rem, 3.4vw, 2.75rem)', { lineHeight: '1', letterSpacing: '-0.015em' }],
        eyebrow: ['0.75rem', { lineHeight: '1rem', letterSpacing: '0.08em' }],
        micro: ['0.6875rem', { lineHeight: '0.875rem', letterSpacing: '0.06em' }],
      },
      maxWidth: {
        site: '80rem',
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
