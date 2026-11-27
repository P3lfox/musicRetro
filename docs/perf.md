# Performance log

Measured per milestone (spec §3, §7). Release profile, idle window.

| Milestone | Date | Platform | Binary | RAM (private dirty) | RAM (RSS) | Idle CPU | Cold start |
|---|---|---|---|---|---|---|---|
| M0 | 2026-11-27 | Linux x86_64, Wayland, FemtoVG/Mesa | 12 MB | ~29 MB | ~79 MB | 0 ticks / 5 s | not measured |
| M1 | 2026-11-27 | Linux x86_64, ALSA → PipeWire, `play` example | — | — | ~9 MB | ~1.2 % playing FLAC 44.1k, 0 paused | — |

## Notes

- **M0:** most of the RSS is file-backed GPU driver code (Mesa, ~50 MB `Private_Clean`/shared),
  not app memory. Re-measure on Windows and compare with Slint's software renderer.
  Idle CPU is zero: the clock only writes its property when the minute changes.
- **M1:** CPU while playing is essentially decoding cost; the engine thread sleeps 40 ms
  between buffer refills (0.5 s ring buffer) and blocks with no wake-ups while paused or stopped.
