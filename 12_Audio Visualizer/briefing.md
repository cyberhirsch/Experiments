# № 12 — Audio Visualizer

**Real-time spectral spiral — a 3D representation of the Short-Time Fourier Transform.**

Each frame captures a window of audio (2048 samples), applies a Hann window,
and computes the FFT to extract **magnitude** and **phase** for every frequency
bin. These are mapped to 3D positions as a spiral:

```
x = f                  (frequency bin index)
y = |X(f)| · cos(ϕ)    (magnitude × cos(phase))
z = |X(f)| · sin(ϕ)    (magnitude × sin(phase))
```

The frequency axis runs horizontally, while the complex amplitude of each bin
winds around it — forming a structure where the spiral's radius at each point
is the magnitude and its angular offset is the phase. Successive audio frames
morph the spiral in real time.

## Why this mapping

Phase is circular (−π and +π are the same angle) and doesn't map naturally to a
straight Z-axis. Encoding it as the angular coordinate of a helix or spiral is
geometrically faithful: two bins with the same magnitude but opposite phases
appear at mirror-image points across the axis, which the eye reads correctly.

The STFT frame rate is implicit — every rendered frame is one FFT window, so
time is the *animation* dimension.

## Audio sources

| Source | Description |
|--------|-------------|
| **Mic** | Live microphone input via `getUserMedia` |
| **File** | Upload any audio file (decode via Web Audio API) |
| **Demo** | Procedural synthetic signal (frequency sweeps with controlled harmonics) |

## Visualization

- **Three.js** Points with additive blending, custom shader for per-vertex size
- **Color modes:** spectral (rainbow), heat (brightness-to-temperature), or
  frequency-index (cool-to-warm gradient)
- **OrbitControls** for free camera rotation — auto-rotate option
- Frequency axis with tick marks; rings/grid helpers can be toggled

## Controls

- **Particle size** — global point size multiplier
- **Spiral radius** — scale the magnitude-to-radius mapping
- **Color mode** — three palette options
- **Source** — switch between demo, mic, and file upload
- **Auto-rotate** — gentle automatic camera motion

## Roadmap / ideas

- Audio worklet for zero-latency FFT pipeline
- Logarithmic frequency mapping (mel scale) for perceptual spacing
- Spectral flux and onset detection for rhythmic visual accents
- Waterfall / persistence mode (trails of previous frames)
- Connected mesh between spiral strands
