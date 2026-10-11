# Vector Toolkit

Geoprocessing tools for sampling and tessellation: hexagon and square grids over a layer's extent, and random points inside polygons.

## Tools

- **Hexagon Grid** (`plugin:vector-toolkit/hexagon-grid`): Covers a layer's extent with hexagons of a given size.
- **Square Grid** (`plugin:vector-toolkit/square-grid`): Covers a layer's extent with square cells (a fishnet).
- **Random Points In Polygons** (`plugin:vector-toolkit/random-points`): Scatters random points inside each polygon.

## Permissions

None: it only reads the map.

## Build

```sh
cargo build --release --target wasm32-unknown-unknown -p vector-toolkit
```
