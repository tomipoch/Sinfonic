import { DEFAULT_THEME_ID, type Theme } from "@/modules/theme/types";
export { DEFAULT_THEME_ID };
import { caffeine } from "@/modules/theme/themes/caffeine";
import { catppuccin } from "@/modules/theme/themes/catppuccin";
import { claude } from "@/modules/theme/themes/claude";
import { dracula } from "@/modules/theme/themes/dracula";
import { everforest } from "@/modules/theme/themes/everforest";
import { gruvbox } from "@/modules/theme/themes/gruvbox";
import { kanagawa } from "@/modules/theme/themes/kanagawa";
import { kanagawaDragon } from "@/modules/theme/themes/kanagawa-dragon";
import { nord } from "@/modules/theme/themes/nord";
import { rosePine } from "@/modules/theme/themes/rose-pine";
import { sage } from "@/modules/theme/themes/sage";
import { sinfonicDefault } from "@/modules/theme/themes/sinfonic-default";
import { solarized } from "@/modules/theme/themes/solarized";
import { tide } from "@/modules/theme/themes/tide";
import { tokyoNight } from "@/modules/theme/themes/tokyo-night";

const BUILTIN: Theme[] = [
  sinfonicDefault,
  claude,
  kanagawa,
  kanagawaDragon,
  tokyoNight,
  catppuccin,
  rosePine,
  everforest,
  nord,
  gruvbox,
  dracula,
  solarized,
  tide,
  sage,
  caffeine,
];

const BY_ID = new Map<string, Theme>(BUILTIN.map((t) => [t.id, t]));

export function listBuiltinThemes(): Theme[] {
  return BUILTIN;
}

export function getBuiltinTheme(id: string): Theme | undefined {
  return BY_ID.get(id);
}

export function getDefaultTheme(): Theme {
  return BY_ID.get(DEFAULT_THEME_ID) ?? BUILTIN[0];
}
