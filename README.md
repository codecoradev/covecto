# Covecto

Dual-engine image vectorization — pixel-exact for icons, smooth curves for art.

## Engines

| Engine | Best For | Algorithm |
|--------|----------|-----------|
| **PixelExact** | Icons, pixel art, screenshots | Contiguous region flood-fill → boundary tracing → rectilinear SVG |
| **Spline** | Photos, illustrations, artwork | Color quantization → contour tracing → Bézier spline fitting |
| **Auto** | Everything | Heuristic selection based on image characteristics |

## CLI

```bash
# Install
cargo install covecto

# Basic (auto engine)
covecto input.png -o output.svg

# Force pixel-exact
covecto icon.png --engine pixel-exact -o icon.svg

# Spline with photo preset
covecto photo.jpg --engine spline --preset photo --optimize -o photo.svg

# Batch
covecto ./icons/ --engine pixel-exact -o ./output/

# Serve API
covecto serve --port 3000

# Optimize existing SVG
covecto optimize input.svg -o output.svg
```

## HTTP API

```bash
# Start server
covecto serve --port 3000

# Vectorize
curl -F "file=@input.png" -F "engine=auto" http://localhost:3000/v1/vectorize

# Optimize
curl -F "file=@input.svg" http://localhost:3000/v1/optimize

# Health
curl http://localhost:3000/v1/health
```

## License

MIT