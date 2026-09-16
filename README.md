# spark

A **spark** is the opener for a family of files. jpg/png/webp → `image`. md/txt → `text`.

Not every particle has a spark. That is the condition for gliding `cyb://particle/` → `cyb://file/`.

This crate is Bevy-free. It returns a [`Surface`]; spacetime mounts it.

```
particle  →  spark::resolve(file)  →  Option<SparkId>
spark::open(file)  →  Option<Surface>
```
