# Performance log

Measured per milestone (spec §3, §7). Release profile, idle window.

| Milestone | Date | Platform | Binary | RAM (private dirty) | RAM (RSS) | Idle CPU | Cold start |
|---|---|---|---|---|---|---|---|
| M0 | 2026-11-27 | Linux x86_64, Wayland, FemtoVG/Mesa | 12 MB | ~29 MB | ~79 MB | 0 ticks / 5 s | not measured |

## Notes

- **M0:** most of the RSS is file-backed GPU driver code (Mesa, ~50 MB `Private_Clean`/shared),
  not app memory. Re-measure on Windows and compare with Slint's software renderer.
  Idle CPU is zero: the clock only writes its property when the minute changes.
