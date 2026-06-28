// `cn` — className composition helper.
//
// Resolves Tailwind utility conflicts via `tailwind-merge` so
// consumers can override utility classes by passing conflicting
// ones via the `className` prop, e.g.:
//
//   <Foo className="bg-blue-500" />          // blue wins
//   <Foo className="bg-red-500 bg-blue-500" /> // last wins (red)
//
// `clsx` still handles the falsy / array / object inputs; `twMerge`
// only deduplicates the resulting Tailwind class list.

import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export const cn = (...inputs: ClassValue[]) => twMerge(clsx(inputs));
