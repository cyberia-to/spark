# spark spec

- A spark draws a [`File`](../../file). It does not name the particle.
- Resolve: sniffed [`Kind`](../../file) → `SparkId`. Graph axons of type (`.avi`) are a later host hint and override sniff.
- Core sparks: `text`, `image`. Later: `audio`, `video`, `table`, `vector`, `prog`.
- No spark → no file page. The particle page is the whole truth.
- `open` never panics on unknown bytes; it returns `None` or `SparkError`.
