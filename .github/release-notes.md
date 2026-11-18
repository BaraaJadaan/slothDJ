## Which file do I need?

| Platform | Download |
| --- | --- |
| **Windows 10/11** | `slothDJ_…_x64-setup.exe` (installer) or `slothDJ_…_x64_portable.exe` (no install) |
| **macOS, Apple Silicon** | `slothDJ_…_aarch64.dmg` |
| **macOS, Intel** | `slothDJ_…_x64.dmg` |
| **Linux** | `slothDJ_…_amd64.AppImage` or `slothDJ_…_amd64.deb` |
| **Browser extension** (Chrome, Edge, Brave, Opera, Vivaldi) | `slothDJ-extension-….zip` |

On first launch slothDJ downloads its engine once (yt-dlp, plus ffmpeg if you don't already have it, about 50 MB).

The builds aren't code-signed yet:
- **Windows:** if SmartScreen appears, click **More info → Run anyway**.
- **macOS:** right-click the app → **Open** the first time. If macOS says it's damaged, run `xattr -dr com.apple.quarantine /Applications/slothDJ.app`.

Extension setup: unzip, open `chrome://extensions`, turn on **Developer mode**, click **Load unpacked** and pick the unzipped folder. Full guide in the [README](https://github.com/BaraaJadaan/slothDJ#browser-extension).
