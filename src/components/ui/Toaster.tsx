// Sonner Toaster wrapper — single instance for the whole app.

import { Toaster as SonnerToaster } from "sonner";

export function Toaster() {
  return <SonnerToaster position="bottom-right" theme="dark" richColors closeButton />;
}
