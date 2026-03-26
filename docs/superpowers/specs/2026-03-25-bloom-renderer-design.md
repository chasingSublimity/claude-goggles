# Bloom Renderer Design Spec

An alternative visualization mode for claude-goggles that represents agents as pulsing, glowing spheres rendered with braille characters, replacing the tree view when `--viz bloom` is passed.

## Problem

The tree view is informative but utilitarian. claude-goggles should feel like generative art, not a dashboard. A bloom visualization mode provides an organic, ambient view of agent activity that encodes status and scale through motion, size, and color.

## Solution

A braille-based particle renderer that draws each agent as a glowing sphere. Spheres cluster together via simple physics (gravity + repulsion), pulse when active, and use additive color blending where their halos overlap.

## CLI Integration

### Flag

```
claude-goggles --viz bloom
```

The `--viz` argument accepts `tree` (default) or `bloom`. Both use the same `Renderer` trait and share the event pipeline, model, and footer.

```rust
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    /// Visualization mode: tree or bloom
    #[arg(long, default_value = "tree")]
    viz: String,
}
```

### Renderer Trait Change

The `Renderer` trait changes from `&self` to `&mut self` to support stateful renderers. `BloomRenderer` maintains simulation state (sphere positions, velocities, phases) that must be mutated each frame. `TreeViewRenderer` is stateless and unaffected by this change — `&mut self` is a superset.

```rust
pub trait Renderer {
    fn render(&mut self, tree: &AgentTree, frame: &mut Frame, scroll_offset: usize, selected: usize);
}
```

### Renderer Toggle

`main.rs` holds both renderers and a mode enum. Sphere state persists across toggles — switching away from bloom and back restores the same positions.

```rust
enum VizMode { Tree, Bloom }

// in run_tui:
let mut tree_renderer = TreeViewRenderer;
let mut bloom_renderer = BloomRenderer::new();
let mut viz_mode = match cli.viz.as_str() {
    "bloom" => VizMode::Bloom,
    _ => VizMode::Tree,
};
```

### Keyboard Controls

- `q` / `Ctrl+C` — quit (both modes)
- `v` — toggle between bloom and tree view live
- `j`/`k`/arrows/`c` — only processed when in tree mode; suppressed in bloom mode

Footer in bloom mode shows: agent count, total tokens, dropped events, "v: tree view". No scroll/collapse hints.

## Rendering Pipeline

Four stages per frame (~10fps, matching existing tick rate). Simulation advances once per render call — physics is tied to the frame tick, not wall-clock time. This is acceptable at 10fps and avoids the complexity of delta-time physics.

### 1. Simulate

Update sphere physics:
- Apply gravity toward canvas center
- Repel overlapping spheres
- Advance pulse phase (sine wave oscillator)
- Apply velocity damping

### 2. Rasterize

For each sphere, compute which braille sub-dots fall within its bloom radius. For each affected dot, calculate color intensity based on distance from sphere center using exponential falloff.

### 3. Composite

Merge all spheres into a single pixel buffer using additive color blending. Where two bloom halos overlap, their RGB values add (clamped to 255), creating natural light-mixing effects.

### 4. Encode

Convert the pixel buffer to terminal cells. For each 2x4 block of dots:
- Set the braille character pattern for any lit dots
- Pick the cell's foreground color as the brightest dot in that block
- Background stays terminal default (black)

### Pixel Buffer

A flat `Vec<(f32, f32, f32)>` at braille resolution:
- Width: `terminal_columns * 2`
- Height: `terminal_rows * 4` (minus footer row)

Cleared to black each frame. On terminal resize, the buffer is reallocated and the canvas center is recomputed. Sphere positions are not clamped — gravity naturally pulls off-screen spheres back toward the new center within a few frames.

## Sphere State

```rust
struct Sphere {
    agent_id: String,
    position: (f32, f32),       // braille-pixel space
    velocity: (f32, f32),
    base_radius: f32,           // from token usage
    pulse_phase: f32,           // 0.0..2π
    color: (u8, u8, u8),       // RGB from palette
    status: SphereStatus,       // Running, Idle, Completed
    fade_start: Option<Instant>, // set to Instant::now() when renderer first observes Completed
}
```

### Physics

Forces applied each tick:
- **Gravity**: `acceleration = 0.02 * (center - position)`. Keeps cluster cohesive.
- **Repulsion**: Between every pair, `acceleration = 0.5 * overlap_amount * direction_away`. Prevents overlap, creates organic spacing. O(n^2) pairwise — assumes fewer than ~50 concurrent agents, at which scale this is negligible relative to rasterization cost.
- **Damping**: `velocity *= 0.9` each tick. Prevents eternal bouncing.

### Radius

`effective_radius = base_radius + pulse_amplitude * sin(pulse_phase)`

| Status | pulse_amplitude | phase_speed | bloom_spread |
|--------|----------------|-------------|-------------|
| Running | 3.0 | 0.15 | 0.8 (wide, soft glow) |
| Idle | 1.0 | 0.05 | 0.5 (moderate) |
| Completed | 0.0 (static) | 0.0 | 0.3 (tight, concentrated) |

**Base radius from tokens:**
```
base_radius = clamp(3.0, sqrt(total_tokens / 500.0) * 4.0, 20.0)
```

- `token_usage` is `None`: treat total tokens as 0, radius = 3 (small bright spark)
- 10k tokens: radius ~11.3
- 50k tokens: radius ~12.6 (diminishing returns via sqrt)

### Spawning

New spheres appear at a random offset from canvas center with a small outward velocity, drifting into place naturally.

## Color

### Palette

8 distinct colors, assigned sequentially as agents appear (wraps after 8):

| Index | Name | RGB |
|-------|------|-----|
| 0 | Cyan | (0, 210, 255) |
| 1 | Magenta | (255, 105, 180) |
| 2 | Gold | (255, 217, 61) |
| 3 | Green | (107, 203, 119) |
| 4 | Coral | (255, 107, 53) |
| 5 | Lavender | (180, 130, 255) |
| 6 | Teal | (0, 200, 170) |
| 7 | Rose | (255, 150, 150) |

Root always gets index 0 (Cyan).

### Bloom Falloff

```
intensity = exp(-distance² / (radius² * bloom_spread))
```

The sphere's palette color is multiplied by intensity at each dot. Full RGB at center, fading smoothly to black at the edge.

### Additive Compositing

```
pixel.r = min(255, sphere_a.r + sphere_b.r)
pixel.g = min(255, sphere_a.g + sphere_b.g)
pixel.b = min(255, sphere_a.b + sphere_b.b)
```

Overlapping cyan + magenta halos create white-ish glow at intersections.

### Completed Fade

After completion, color fades over 3 seconds:
```
color_multiplier = max(0.2, 1.0 - fade_elapsed_secs / 3.0)
```

Settles at 20% brightness — still visible but clearly done.

## Agent Sync

Each frame, `BloomRenderer` reconciles its sphere list with `AgentTree`:
- New agent IDs in the tree get new spheres (assigned next palette color)
- Agents that transition to Completed get their sphere status updated and `fade_start` set to `Instant::now()`
- Sphere `base_radius` is updated from current `token_usage` each frame
- Sphere `status` is updated from current `AgentStatus` each frame

Tracked via `known_agents: HashSet<String>` on the renderer.

### Flat Traversal

The sync step needs to visit all agents regardless of collapsed state. A new method is added to `Agent` in the model:

```rust
impl Agent {
    /// Collect all agents in the tree as a flat list (depth-first).
    pub fn all_agents(&self) -> Vec<&Agent> {
        let mut result = vec![self];
        for child in &self.children {
            result.extend(child.all_agents());
        }
        result
    }
}
```

This is a read-only traversal that fits cleanly in `model/` and is useful to any renderer that needs a flat view.

### Empty Tree

When the tree has no root (no events received yet), bloom mode renders a blank canvas with only the footer. No placeholder text — the empty dark screen is intentional.

## Braille Encoding

Unicode braille characters (U+2800..U+28FF) encode a 2x4 dot matrix per cell. Each of the 8 dots maps to a bit:

```
Dot 1 (0x01)  Dot 4 (0x08)
Dot 2 (0x02)  Dot 5 (0x10)
Dot 3 (0x04)  Dot 6 (0x20)
Dot 7 (0x40)  Dot 8 (0x80)
```

Character = `'\u{2800}' + dot_bits`

A dot is "lit" if its corresponding pixel in the buffer has intensity above a threshold (e.g., 0.05). The cell's foreground color is the max-intensity pixel's color within that 2x4 block.

## Module Structure

### New Files

- `src/render/bloom.rs` — `BloomRenderer`, `Sphere`, simulation, rasterization, braille encoding. Single file, targeting ~350-400 lines.

### Modified Files

- `src/render/mod.rs` — add `pub mod bloom;`, change `Renderer` trait to `&mut self`
- `src/render/tree_view.rs` — update `impl Renderer` signature to `&mut self` (no logic changes)
- `src/model/mod.rs` — add `Agent::all_agents()` method for flat traversal
- `src/main.rs` — add `--viz` flag to `Cli` struct, hold both renderers, mode enum, `v` key toggle, suppress j/k/c keys in bloom mode

### Unchanged

`events/`, `cli/` — no changes. The `render/ ↔ model/` boundary is preserved — `model/` gains a read-only traversal helper, not a render dependency.

## Testing

Physics and color math are pure functions, tested independently:

| Test | What it verifies |
|------|-----------------|
| `test_bloom_falloff` | Intensity ~1.0 at center, near 0 at edge, negligible beyond radius |
| `test_additive_blend` | Two colors add correctly, clamp at 255 |
| `test_repulsion_separates_spheres` | Two overlapping spheres drift apart after N ticks |
| `test_gravity_pulls_toward_center` | Sphere far from center moves closer after N ticks |
| `test_radius_from_tokens` | 0 tokens = min radius, large tokens = clamped max |
| `test_braille_encoding` | Known dot pattern produces correct braille character |
| `test_sphere_sync` | New agent in tree creates sphere; IDs tracked correctly |

The full simulation + rasterization pipeline is validated visually rather than via snapshot tests, since output depends on terminal size and timing.
