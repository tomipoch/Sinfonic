import type { Theme } from "@/modules/theme/types";

export const sinfonicDefault: Theme = {
  id: "sinfonic-default",
  name: "Sinfonic Default",
  description: "The default Sinfonic look — Spotify-inspired dark with emerald accent.",
  editorTheme: { dark: "atomone", light: "atomone" },
  variants: {
    light: {},
    dark: {},
  },
};
