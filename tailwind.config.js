/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Dark theme defaults — Sinfonic ships with a Spotify-like look
        bg: {
          DEFAULT: "#0b0b0e",
          subtle: "#15151a",
          raised: "#1f1f25",
        },
        fg: {
          DEFAULT: "#f5f5f7",
          subtle: "#a1a1aa",
          muted: "#71717a",
        },
        accent: {
          DEFAULT: "#10b981",
          hover: "#34d399",
        },
      },
      fontFamily: {
        sans: [
          "system-ui",
          "-apple-system",
          "Segoe UI",
          "Roboto",
          "Helvetica Neue",
          "Arial",
          "sans-serif",
        ],
      },
    },
  },
  plugins: [],
};
