# Long-term goals

Owner feature goals for the TSFP port. Tick an item when it ships default-on; link its task when work starts.
Faithful-core rules still apply: new behaviour that changes simulation outcomes is opt-in and labelled.

## Launcher (T4330)

- [ ] Auto-update for launcher and game (T4333)
- [ ] Guided setup wizard (T4331 core, T4332 UI)
  - [ ] Pick or drop the disc image; identify it by hash
  - [ ] Optional high-quality music pack (T4334)
  - [ ] Optional high-quality texture pack (T4334)
  - [ ] Verify every installed file against its hash; repair on mismatch
- [ ] Game settings editable from the launcher, shared with the in-game menu (T4332)
- [ ] Mods tab: enable, disable, load order (T4335)
- [ ] Linux, macOS and Windows on x86-64 and ARM64 (T4336)

## Settings

- [ ] In-game settings menus: video, audio, controls, gameplay

## Display and performance

- [ ] Widescreen and ultrawide support
- [ ] Simulation decoupled from rendering: render above 60 FPS with faster input sampling, simulation stays at 60 Hz

## Input

- [ ] Native keyboard and mouse, including local multiplayer
  - [ ] Own bindings, not a 1:1 controller mapping (actions that share a controller button can be split)
  - [ ] Defaults: `WASD` move, mouse aim, `LMB` fire, `RMB` aim, `MMB` weapon alt mode, wheel cycle weapons,
        `1`-`9` weapon slot, `R` reload, `Esc` pause
  - [ ] Wheel zooms while scoped
- [ ] Menu navigation per device
  - [ ] Mouse: lists scroll only with the wheel, never by hovering at the edges
  - [ ] Controller: lists auto-scroll at the edges
  - [ ] Keyboard and controller selection always scrolls the selected item into view

## Profiles

- [ ] Players join from the main screen without choosing a player slot or profile
- [ ] One profile per machine, default to the last used; changeable

## Multiplayer and co-op

- [ ] Native direct-IP networking
- [ ] Online co-op
- [ ] Restore companion events in co-op (the special actions the AI companion performs in single player)
- [ ] Remove the co-op teleport when players are too far apart
- [ ] Synced movies and music
  - [ ] Movies start only when every player is ready
  - [ ] Skip by vote: every player holds a button to skip
  - [ ] Minimal network traffic

## Assets and audio

- [ ] Music override that plays the high-quality pack
- [ ] Higher-quality assets from the PS2, GameCube and other versions

## Mapmaker

- [ ] Remove limits on object count and grid / world size
- [ ] Standard `.tsfpmap` file format
  - [ ] Open and save with the native OS file picker
  - [ ] Open a map file directly; OS file association (double-click opens Mapmaker)
  - [ ] Convert maps from the Xbox, PS2 and GameCube versions
- [ ] Autosave and crash recovery
- [ ] Launch Mapmaker directly, skipping menus (default profile)

## AI

- [ ] AI improvements (opt-in)
