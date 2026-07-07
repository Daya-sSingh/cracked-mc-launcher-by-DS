import type { Config } from "tailwindcss";

const config: Config = {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Base surfaces (darkest → elevated)
        base:     "#0f1014",
        surface:  "#16181d",
        elevated: "#1d2027",
        overlay:  "#242731",
        // Border
        border:   "rgba(255,255,255,0.07)",
        // Text hierarchy
        primary:  "#e8eaf0",
        secondary: "#8b8fa8",
        muted:    "#4a4d62",
        // Ember accent (matches the icon palette)
        accent: {
          DEFAULT: "#ff9a57",
          hover:   "#ffb07a",
          muted:   "rgba(255,154,87,0.15)",
          dim:     "rgba(255,154,87,0.08)",
        },
        // Status colours
        success: "#4caf84",
        warning: "#f5a623",
        danger:  "#ff5f5f",
        info:    "#5b8af5",
        // Fabric loader badge
        fabric:  "#dbb263",
      },
      fontFamily: {
        sans: [
          "Inter",
          "system-ui",
          "-apple-system",
          "BlinkMacSystemFont",
          "Segoe UI",
          "sans-serif",
        ],
        mono: ["JetBrains Mono", "Fira Code", "Cascadia Code", "monospace"],
      },
      borderRadius: {
        sm:  "6px",
        DEFAULT: "10px",
        md:  "12px",
        lg:  "16px",
        xl:  "20px",
        "2xl": "24px",
      },
      boxShadow: {
        sm:   "0 1px 3px rgba(0,0,0,0.4)",
        DEFAULT: "0 4px 12px rgba(0,0,0,0.5)",
        lg:   "0 8px 24px rgba(0,0,0,0.6)",
        xl:   "0 16px 40px rgba(0,0,0,0.7)",
        glow: "0 0 20px rgba(255,154,87,0.25)",
      },
      animation: {
        "fade-in":    "fadeIn 0.15s ease-out",
        "slide-up":   "slideUp 0.2s ease-out",
        "slide-down": "slideDown 0.2s ease-out",
        "scale-in":   "scaleIn 0.15s ease-out",
        "spin-slow":  "spin 2s linear infinite",
        "pulse-soft": "pulseSoft 2s ease-in-out infinite",
      },
      keyframes: {
        fadeIn:    { from: { opacity: "0" },                 to: { opacity: "1" } },
        slideUp:   { from: { opacity: "0", transform: "translateY(8px)" },  to: { opacity: "1", transform: "translateY(0)" } },
        slideDown: { from: { opacity: "0", transform: "translateY(-8px)" }, to: { opacity: "1", transform: "translateY(0)" } },
        scaleIn:   { from: { opacity: "0", transform: "scale(0.95)" },      to: { opacity: "1", transform: "scale(1)" } },
        pulseSoft: {
          "0%, 100%": { opacity: "1" },
          "50%":      { opacity: "0.6" },
        },
      },
      transitionTimingFunction: {
        spring: "cubic-bezier(0.34, 1.56, 0.64, 1)",
      },
    },
  },
  plugins: [],
};

export default config;
