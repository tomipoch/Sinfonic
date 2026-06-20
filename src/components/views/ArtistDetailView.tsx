// Phase 0 placeholder. Real implementation in Phase 7.

import { useParams } from "react-router-dom";

export function ArtistDetailView() {
  const { id } = useParams<{ id: string }>();
  return (
    <section className="p-6">
      <h1 className="mb-2 text-2xl font-semibold">Artist</h1>
      <p className="text-fg-subtle">Artist id: {id}</p>
    </section>
  );
}
