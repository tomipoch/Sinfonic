// `defineTheme()` — sugar over the `Theme` shape so each theme file
// doesn't have to wrap its variants under a `variants:` key.
//
// Before:
//
//   export const caffeine: Theme = {
//     id: "caffeine",
//     name: "Caffeine",
//     editorTheme: { dark: "xcode-dark", light: "xcode-light" },
//     variants: {
//       dark: { colors: { … }, terminal: { … } },
//       light: { colors: { … }, terminal: { … } },
//     },
//   };
//
// After:
//
//   export const caffeine = defineTheme({
//     id: "caffeine",
//     name: "Caffeine",
//     editorTheme: { dark: "xcode-dark", light: "xcode-light" },
//     dark: { colors: { … }, terminal: { … } },
//     light: { colors: { … }, terminal: { … } },
//   });
//
// The function does NOT derive any color values from each other —
// each theme still specifies every field explicitly. The savings is
// the wrapper un-nesting (~2 lines per theme × 14 = ~28 lines) and,
// more importantly, the structural clarity: `dark:` / `light:` are
// at the top level, where you read them.
//
// `sinfonic-default` (the empty-theme) is still writable as:
//
//   export const sinfonicDefault = defineTheme({ id, name, editorTheme });
//
// — both `dark` and `light` are optional.

import type { Theme, ThemeVariant } from "./types";

interface DefineThemeInput {
  id: string;
  name: string;
  description?: string;
  author?: string;
  editorTheme?: Theme["editorTheme"];
  dark?: ThemeVariant;
  light?: ThemeVariant;
}

export function defineTheme(input: DefineThemeInput): Theme {
  return {
    id: input.id,
    name: input.name,
    description: input.description,
    author: input.author,
    editorTheme: input.editorTheme,
    variants: {
      dark: input.dark,
      light: input.light,
    },
  };
}
