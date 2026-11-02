# Long-term goals

Feature goals for the TSFP port. Status: **Done** = works today, **Partial** = parts work (detail in brackets),
**Planned** = not started. An item is ticked only when it is fully done and on by default. Gameplay-changing
behaviour stays opt-in and labelled.

Last reviewed: 2026-11-02.

## Launcher (T4330)

- [x] **Done.** Desktop launcher for Linux, macOS and Windows on x64 and ARM64, built by CI on every platform (T4332)
- [ ] **Partial.** Auto-update: checks GitHub Releases per channel, installs only signed builds (T4333)
      [waits for the release signing key to be added to the repo]
- [x] **Done.** Guided setup: pick or drop the disc image, identify it by hash, extract and verify (T4331, T4332)
  - [ ] **Planned.** Optional high-quality music pack (T4334)
  - [ ] **Planned.** Optional high-quality texture pack (T4334)
  - [x] **Done.** Verify every installed file against its hash; damaged installs flagged for repair
- [x] **Done.** Game settings editable in the launcher, shared with the in-game menu file (T4332)
- [x] **Done.** Mods tab: install from zip, enable, load order, conflicts, remove (T4335)

## Settings

- [ ] **Partial.** In-game settings menus: video, audio, controls, gameplay
      [shared settings schema and launcher editor done; in-game screens T1697, T2211]

## Display and performance

- [ ] **Partial.** Widescreen and ultrawide
      [aspect override, Hor+ camera, HUD edge anchoring done (T2094, T2171, T2285, T2286); ultrawide cutscenes and
      movies open (T2170, T2096)]
- [ ] **Partial.** Render above 60 FPS with simulation fixed at 60 Hz
      [display-rate HUD, camera and interpolation snapshots done (T2330, T2334, T2336); full render loop open (T2244)]

## Input

- [ ] **Partial.** Native keyboard and mouse, including local multiplayer
      [keyboard and mouse work in menus, Map Maker and gameplay; full rebinding system open (T3890); mixed split-screen
      open (T2164)]
  - [ ] **Planned.** Own bindings, not a 1:1 controller mapping (actions that share a controller button can be split)
  - [ ] **Planned.** Defaults: `WASD` move, mouse aim, `LMB` fire, `RMB` aim, `MMB` weapon alt mode, wheel cycle
        weapons, `1`-`9` weapon slot, `R` reload, `Esc` pause
  - [ ] **Planned.** Wheel zooms while scoped
- [ ] **Partial.** Menu navigation per device
      [native focus, hover, click and wheel scroll done (T2308); per-device scrolling standard open (T2327)]
  - [ ] Mouse: lists scroll only with the wheel, never by hovering at the edges
  - [ ] Controller: lists auto-scroll at the edges
  - [ ] Keyboard and controller selection always scrolls the selected item into view

## Profiles

- [ ] **Planned.** Players join from the main screen without choosing a player slot or profile
- [ ] **Planned.** One profile per machine, default to the last used; changeable

## Multiplayer and co-op

- [ ] **Partial.** Native direct-IP networking [direct-IP transport in progress (T3891)]
- [ ] **Partial.** Online co-op [System Link co-op investigated (T2039); host-authoritative design open (T2152)]
- [ ] **Planned.** Restore companion events in co-op (the special actions the AI companion performs in single player)
- [ ] **Planned.** Remove the co-op teleport when players are too far apart
- [ ] **Planned.** Synced movies and music (T2156)
  - [ ] Movies start only when every player is ready
  - [ ] Skip by vote: every player holds a button to skip
  - [ ] Minimal network traffic

## Assets and audio

- [ ] **Partial.** Asset overrides [lossless extraction and the override folder contract done (`tsfp-import`); music
      and texture packs planned (T4334)]
- [ ] **Planned.** Higher-quality assets from the PS2, GameCube and other versions (the importer reads all three
      formats)

## Mapmaker

- [ ] **Planned.** Remove limits on object count and grid / world size
- [ ] **Partial.** Standard `.tsfpmap` file format [save and storage functions ported to Rust (T1919); file format
      and picker open (T2362)]
  - [ ] Open and save with the native OS file picker
  - [ ] Open a map file directly; OS file association (double-click opens Mapmaker)
  - [ ] Convert maps from the Xbox, PS2 and GameCube versions
- [ ] **Planned.** Autosave and crash recovery
- [ ] **Planned.** Launch Mapmaker directly, skipping menus (default profile)

## AI

- [ ] **Planned.** AI improvements (opt-in)
