# Hello RustGIS

A starter plugin to copy: commands that report on the map's layers, zoom to the largest one and drop a pin at the map's centre, and a tool that draws bounding boxes.

## Commands

- **Layer Report** (`plugin:hello-rustgis/layer-report`): Count the map's layers and features, and show the summary in the status bar.
- **Zoom To Largest** (`plugin:hello-rustgis/zoom-largest`): Zoom to the layer with the most features.
- **Pin Center** (`plugin:hello-rustgis/pin-center`): Add a point at the centre of the map to a new Pins layer.

## Tools

- **Bounding Boxes** (`plugin:hello-rustgis/bounding-boxes`): Draws the bounding box of each feature, or of the whole layer.

## Permissions

edit

## Build

```sh
cargo build --release --target wasm32-unknown-unknown -p hello-rustgis
```
