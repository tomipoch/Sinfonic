// Tiny `className` helper. KISS: re-export of `clsx` keeps the import
// short and lets us swap the underlying lib later without touching
// call sites.

import { type ClassValue, clsx } from "clsx";

export const cn = (...inputs: ClassValue[]) => clsx(inputs);
