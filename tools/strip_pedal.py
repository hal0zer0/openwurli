#!/usr/bin/env python3
"""Strip sustain pedal (CC64) from MIDI files, extending note durations to compensate.

When sustain pedal is active, note-offs are deferred until pedal-off. This script
simulates that behavior by extending note durations, then removes the CC64 events.
Originals are kept; stripped versions go to a _nopedal.mid suffix.

Usage:
    python tools/strip_pedal.py midis/           # process all .mid files in directory
    python tools/strip_pedal.py midis/foo.mid    # process a single file
"""

import sys
import os
import copy
import mido


def strip_pedal(input_path: str, output_path: str) -> dict:
    """Strip CC64, extending note durations where pedal was held.

    Returns stats dict with counts of modifications.
    """
    mid = mido.MidiFile(input_path)
    out = mido.MidiFile(ticks_per_beat=mid.ticks_per_beat, type=mid.type)

    total_pedal_events = 0
    total_notes_extended = 0

    for track in mid.tracks:
        new_track = mido.MidiTrack()

        # Convert to absolute time for easier manipulation
        abs_msgs = []
        tick = 0
        for msg in track:
            tick += msg.time
            abs_msgs.append((tick, msg))

        # Track pedal state and queued note-offs per channel
        pedal_on = {}          # channel -> bool
        queued_offs = {}       # channel -> list of (original_tick, note, velocity)
        result_msgs = []       # (abs_tick, msg) without pedal events

        for abs_tick, msg in abs_msgs:
            if msg.type == 'control_change' and msg.control == 64:
                total_pedal_events += 1
                ch = msg.channel

                if msg.value >= 64:
                    # Pedal down
                    pedal_on[ch] = True
                else:
                    # Pedal up — flush queued note-offs at this tick
                    pedal_on[ch] = False
                    if ch in queued_offs:
                        for (_orig_tick, note, vel) in queued_offs[ch]:
                            off_msg = mido.Message('note_off', channel=ch,
                                                   note=note, velocity=vel, time=0)
                            result_msgs.append((abs_tick, off_msg))
                            total_notes_extended += 1
                        queued_offs[ch] = []
                continue

            is_note_off = (msg.type == 'note_off' or
                           (msg.type == 'note_on' and msg.velocity == 0))

            if is_note_off and pedal_on.get(msg.channel, False):
                # Queue this note-off until pedal releases
                if msg.channel not in queued_offs:
                    queued_offs[msg.channel] = []
                queued_offs[msg.channel].append((abs_tick, msg.note,
                                                  getattr(msg, 'velocity', 0)))
                continue

            result_msgs.append((abs_tick, msg.copy()))

        # Flush any remaining queued note-offs at end
        for ch, offs in queued_offs.items():
            for (_orig_tick, note, vel) in offs:
                last_tick = abs_msgs[-1][0] if abs_msgs else 0
                off_msg = mido.Message('note_off', channel=ch,
                                       note=note, velocity=vel, time=0)
                result_msgs.append((last_tick, off_msg))
                total_notes_extended += 1

        # Sort by absolute tick (stable sort preserves msg order within same tick)
        result_msgs.sort(key=lambda x: x[0])

        # Convert back to delta time
        prev_tick = 0
        for abs_tick, msg in result_msgs:
            msg_copy = msg.copy()
            msg_copy.time = abs_tick - prev_tick
            new_track.append(msg_copy)
            prev_tick = abs_tick

        out.tracks.append(new_track)

    out.save(output_path)
    return {
        'pedal_events_removed': total_pedal_events,
        'notes_extended': total_notes_extended,
    }


def process_file(path: str):
    base, ext = os.path.splitext(path)
    if '_nopedal' in base:
        return  # skip already-processed files
    out_path = f"{base}_nopedal{ext}"
    stats = strip_pedal(path, out_path)
    if stats['pedal_events_removed'] == 0:
        os.remove(out_path)
        print(f"  {os.path.basename(path)}: no pedal events, skipped")
    else:
        print(f"  {os.path.basename(path)}: removed {stats['pedal_events_removed']} pedal events, "
              f"extended {stats['notes_extended']} notes -> {os.path.basename(out_path)}")


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    target = sys.argv[1]
    if os.path.isdir(target):
        files = sorted(f for f in os.listdir(target)
                       if f.endswith('.mid') and '_nopedal' not in f)
        print(f"Processing {len(files)} MIDI files in {target}:")
        for f in files:
            process_file(os.path.join(target, f))
    else:
        process_file(target)


if __name__ == '__main__':
    main()
