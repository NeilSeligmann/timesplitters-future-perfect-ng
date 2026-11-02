# Troubleshooting

## Launcher

- **Windows says the app is unrecognised.** The launcher is not code-signed yet: *More info → Run anyway*.
- **macOS says the app cannot be opened.** Right-click the app → *Open* once.
- **Linux AppImage does nothing.** Make it executable: `chmod +x tsfp-launcher-linux-x64.AppImage`.
- **"unknown dump".** Your disc's `default.xbe` hash is not one the project has tested. You can still install it.
- **"no default.xbe found".** The file or folder is not an Xbox game disc, or the extracted folder is incomplete.
- **Verify reports damage.** Run *Setup* again with the same disc to repair.
- **Update check failed.** Check your connection; GitHub may also rate-limit frequent checks. Try again later.

## Building from source

- **Disc not found:** put your image in `discs/` or set `TSFP_DISC`.
- **XBE extraction failed:** use an original, readable Xbox game ISO.
- **Tool missing:** install the tools listed in [Building](development/building.md) and retry.
- **Build out of memory:** close other build jobs and retry.
- **No display:** visible output needs a supported Vulkan display.

Never share disc images, extracted game files or game screenshots in issues.
