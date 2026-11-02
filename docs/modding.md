# Modding

A mod is a folder (or a zip of one) with a `mod.json` and a `files/` tree:

```
my-mod/
  mod.json
  files/
    textures/hud_ammo.ktx2
    audio/menu_theme.flac
```

`files/` mirrors the logical asset ids the game uses (`<category>/<name>`, as produced by the importer). A file there
replaces the asset with the same id.

## mod.json

```json
{
  "schema": 1,
  "id": "my-mod",
  "name": "My Mod",
  "version": "1.0.0",
  "description": "Short description shown in the launcher.",
  "game_versions": ["xbox-usa"],
  "affects_gameplay": false
}
```

| Field | Required | Notes |
| --- | --- | --- |
| `schema` | yes | Always `1` for now. |
| `id` | yes | 1-64 characters of `a-z`, `0-9`, `-`, `_`. Must equal the folder name. |
| `version` | yes | Free text, shown in the launcher. |
| `name`, `description` | no | Shown in the launcher. |
| `game_versions` | no | Game versions the mod supports; empty means any. |
| `affects_gameplay` | no | Set `true` if the mod changes gameplay data (weapons, AI, physics, scripts). Such mods are labelled. |

## Load order and conflicts

Enabled mods load in the order shown in the launcher; when two mods provide the same file, the later one wins and the
launcher lists the conflict. On *Save* the launcher writes `mods/active.json` with the load order and the winning mod
for each file; the game reads only that file.

## Packaging

Zip the mod folder (or its contents). Archives with paths that escape the mod folder, and symlinks, are refused.
Do not include original game files in a mod.
