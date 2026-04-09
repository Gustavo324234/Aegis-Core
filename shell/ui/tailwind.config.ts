/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        aegis: {
          black: "#000000",
          navy: "#0d1117",
          cyan: "#00f2fe",
          purple: "#bf00ff",
          steel: "#161b22",
        },
      },
      fontFamily: {
        mono: ["JetBrains Mono", "Fira Code", "ui-monospace", "SFMono-Regular", "Menlo", "Monaco", "Consolas", "Liberation Mono", "Courier New", "monospace"],
        sans: ["Inter", "ui-sans-serif", "system-ui", "-apple-system", "BlinkMacSystemFont", "Segoe UI", "Roboto", "Helvetica Neue", "Arial", "Noto Sans", "sans-serif", "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"],
      },
      backgroundImage: {
        'glass-gradient': 'linear-gradient(135deg, rgba(255, 255, 255, 0.05) 0%, rgba(255, 255, 255, 0) 100%)',
      },
      backdropBlur: {
        xs: '2px',
      }
    },
  },
  plugins: [],
}
