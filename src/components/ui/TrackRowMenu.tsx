// TrackRowMenu — 3-dot context menu for a single track row.
//
// Renders the default per-track actions (queue / playlist / album /
// artist). Views that need extra destructive actions (e.g. Remove
// from playlist) pass them via `extraItems` — they're appended after
// a separator so the menu stays predictable.

import {
  AlbumIcon,
  MoreVerticalIcon,
  MusicNoteSquare01Icon,
  QueueIcon,
  UserCircleIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useState } from "react";
import { toast } from "sonner";
import { AddToPlaylistDialog } from "@/components/dialogs/AddToPlaylistDialog";
import { DropdownMenu, type DropdownMenuItem } from "@/components/ui/DropdownMenu";
import { extractError } from "@/lib/errors";
import { queueAddMany } from "@/lib/tauri";
import type { Track } from "@/types/domain";

interface TrackRowMenuProps {
  track: Track;
  /** Optional extra items appended below a separator. */
  extraItems?: DropdownMenuItem[];
  /** Disable the menu trigger (e.g. while the row is mid-drag). */
  disabled?: boolean;
}

export function TrackRowMenu({ track, extraItems }: TrackRowMenuProps) {
  const [playlistDialogOpen, setPlaylistDialogOpen] = useState(false);

  const onAddToQueue = async () => {
    try {
      await queueAddMany([track]);
      toast.success(`Added "${track.title}" to queue`);
    } catch (err) {
      toast.error(`Couldn't add to queue: ${extractError(err, "unknown error")}`);
    }
  };

  const baseItems: DropdownMenuItem[] = [
    {
      label: "Add to queue",
      icon: <HugeiconsIcon icon={QueueIcon} size={14} strokeWidth={1.75} />,
      onClick: onAddToQueue,
    },
    {
      label: "Add to playlist",
      icon: <HugeiconsIcon icon={MusicNoteSquare01Icon} size={14} strokeWidth={1.75} />,
      onClick: () => setPlaylistDialogOpen(true),
    },
    ...(extraItems && extraItems.length > 0
      ? [{ label: "__separator__", separator: true } as DropdownMenuItem, ...extraItems]
      : []),
    { label: "__separator__", separator: true },
    {
      label: "Go to album",
      icon: <HugeiconsIcon icon={AlbumIcon} size={14} strokeWidth={1.75} />,
      href: track.albumId ? `/albums/${encodeURIComponent(track.albumId)}` : undefined,
      disabled: !track.albumId,
    },
    {
      label: "Go to artist",
      icon: <HugeiconsIcon icon={UserCircleIcon} size={14} strokeWidth={1.75} />,
      href: track.artistId ? `/artists/${encodeURIComponent(track.artistId)}` : undefined,
      disabled: !track.artistId,
    },
  ];

  return (
    <>
      <DropdownMenu
        ariaLabel={`Actions for ${track.title}`}
        trigger={<HugeiconsIcon icon={MoreVerticalIcon} size={16} strokeWidth={1.75} aria-hidden />}
        items={baseItems}
      />
      {playlistDialogOpen && (
        <AddToPlaylistDialog trackIds={[track.id]} onClose={() => setPlaylistDialogOpen(false)} />
      )}
    </>
  );
}
