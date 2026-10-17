# spark spec

- A spark draws a [`File`](../../file). It does not name the particle.
- Resolve: sniffed [`Kind`](../../file) → `SparkId`. Graph axons of type (`.avi`) are a later host hint and override sniff.
- Core sparks: `text`, `image`. Later: `audio`, `video`, `table`, `vector`, `prog`.
- No spark → no file page. The particle page is the whole truth.
- `open` never panics on unknown bytes; it returns `None` or `SparkError`.
- Header readers, no decode and no panic on hostile bytes: `image` (PNG/GIF/JPEG), `webp`, `bmp`, `ico`, `tiff`, `svg`, `heif` dimensions; `gif_duration`, `audio` (WAV), `video` (MP4/MOV `mvhd`), `flac`, `ogg` (Vorbis), `mp3` (MPEG-1 Layer III) durations; `pdf` page count, `pdf_size` first MediaBox, `pdf_text` literal runs of uncompressed content streams. Each returns `None` when the fact is not in the bytes.
