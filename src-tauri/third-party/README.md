# Third-party notices

Omni Fetch invokes the following separately distributed command-line programs:

- **yt-dlp** — distributed under the Unlicense. Official Windows executables also contain components under licenses documented in the `THIRD_PARTY_LICENSES.txt` shipped by the yt-dlp release.
- **gallery-dl** — distributed under GPL version 2 only. Omni Fetch invokes its separately distributed Windows executable for Instagram posts, carousels, images, and videos. Source and license information are available from <https://github.com/mikf/gallery-dl> and <https://codeberg.org/mikf/gallery-dl>.
- **FFmpeg essentials build by Gyan Doshi** — distributed under GPL version 3 or later. The corresponding FFmpeg source is available from <https://ffmpeg.org/download.html> and build information from <https://www.gyan.dev/ffmpeg/builds/>.

Release packaging must include the exact upstream license files accompanying the binary versions selected in `tools-manifest.json`. These programs are not linked into the Omni Fetch process; they run as separate child processes.
