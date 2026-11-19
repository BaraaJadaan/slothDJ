## Which file do I need?

| Platform | Download |
| --- | --- |
| **Windows 10/11** | `SlothDJ_…_x64-setup.exe` (installer) or `SlothDJ_…_x64_portable.exe` (no install) |
| **macOS, Apple Silicon** | `SlothDJ_…_aarch64.dmg` |
| **macOS, Intel** | `SlothDJ_…_x64.dmg` |
| **Linux** | `SlothDJ_…_amd64.AppImage` or `SlothDJ_…_amd64.deb` |
| **Browser extension** (Chrome, Edge, Brave, Opera, Vivaldi) | `SlothDJ-extension-….zip` |

Already on v1.1.0 or newer? SlothDJ updates itself: open it and click **Update now** on the banner.

On first launch SlothDJ downloads its engine once (yt-dlp, plus ffmpeg if you don't already have it, about 50 MB). YouTube also uses Node.js or Deno; if neither is installed, SlothDJ downloads Deno (about 40 MB) the first time you download from YouTube.

The builds aren't code-signed yet:
- **Windows:** if SmartScreen appears, click **More info → Run anyway**.
- **macOS:** right-click the app → **Open** the first time. If macOS says it's damaged, run `xattr -dr com.apple.quarantine /Applications/SlothDJ.app`.

Browser extension: in SlothDJ, click **Add the browser extension** (or Settings → Browser extension → Set up) and follow the short guide. The zip above is for setting it up by hand; see the [README](https://github.com/BaraaJadaan/slothDJ#browser-extension).
