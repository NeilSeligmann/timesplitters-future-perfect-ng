# Verifying downloads

Every asset in a release has a `.sha256` checksum file and a `.minisig` signature made with the project's release
key. Platforms are published one by one as their builds finish; once all are in, a combined `SHA256SUMS` (also
signed) lists every asset.

## Checksums

Download the asset and `SHA256SUMS` into one folder, then:

```sh
sha256sum --ignore-missing -c SHA256SUMS          # Linux
shasum -a 256 --ignore-missing -c SHA256SUMS      # macOS
```

On Windows (PowerShell), compare the output with the line in `SHA256SUMS`:

```powershell
Get-FileHash .\tsfp-launcher-windows-x64.exe -Algorithm SHA256
```

## Signatures

Install [minisign](https://jedisct1.github.io/minisign/) and check a file against the release public key:

```sh
minisign -Vm tsfp-launcher-linux-x64.AppImage -P RWQvJv52Ih8oGINpp9v/f7iSFCtpZF4GOij50+r9qD1B4oKbWM1il22U
```

The launcher's built-in updater performs the same check and never installs an update that fails it.
Releases before v0.1.2 were published unsigned.
