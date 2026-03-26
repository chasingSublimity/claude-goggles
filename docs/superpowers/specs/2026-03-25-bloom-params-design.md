# Real-time Bloom Parameter Tuning

## Problem

The bloom renderer has hardcoded visual constants (sphere size, bloom spread, pulse amplitude, gravity, repulsion). Tweaking these requires recompilation. Users should be able to tune them live to find aesthetics they like.

## Solution

A parameter selector + adjust control scheme in bloom mode. Users cycle through parameters with `[`/`]`, nudge values with `+`/`-`, and reset with `r`. The active parameter and its value are shown in the footer.

## Controls

Active only in bloom mode:

| Key | Action |
|-----|--------|
| `[` / `]` | Cycle selected parameter |
| `+` / `=` / `-` | Nudge selected parameter up/down |
| `r` | Reset all parameters to defaults |

## Tunable Parameters

| # | Name | Field | Default | Step | Min | Max |
|---|------|-------|---------|------|-----|-----|
| 1 | Sphere size (min radius) | `radius_min` | 12.0 | 2.0 | 4.0 | 60.0 |
| 2 | Sphere size (max radius) | `radius_max` | 45.0 | 2.0 | 4.0 | 80.0 |
| 3 | Bloom spread (running) | `bloom_spread_running` | 1.2 | 0.1 | 0.1 | 3.0 |
| 4 | Bloom spread (idle) | `bloom_spread_idle` | 0.8 | 0.1 | 0.1 | 3.0 |
| 5 | Pulse amplitude (running) | `pulse_amp_running` | 5.0 | 0.5 | 0.0 | 15.0 |
| 6 | Pulse amplitude (idle) | `pulse_amp_idle` | 2.0 | 0.5 | 0.0 | 15.0 |
| 7 | Gravity strength | `gravity` | 0.02 | 0.005 | 0.0 | 0.1 |
| 8 | Repulsion padding | `repulsion_padding` | 10.0 | 2.0 | 0.0 | 40.0 |

Min/max radius are split into two parameters so users can control the range independently. Bloom spread and pulse amplitude are split by status (running vs idle) since they have distinct defaults. Completed status values are derived (spread = idle * 0.625, amplitude = 0).

## Implementation

### BloomParams struct

New struct in `render/bloom.rs`:

```rust
pub struct BloomParams {
    pub radius_min: f32,
    pub radius_max: f32,
    pub bloom_spread_running: f32,
    pub bloom_spread_idle: f32,
    pub pulse_amp_running: f32,
    pub pulse_amp_idle: f32,
    pub gravity: f32,
    pub repulsion_padding: f32,
    pub selected: usize,  // 0-7, which parameter is active
}
```

`BloomParams::default()` returns the defaults from the table above. `BloomParams::nudge(up: bool)` adjusts the selected parameter by its step size, clamping to range. `BloomParams::reset()` restores defaults. `BloomParams::param_name(&self) -> &str` and `BloomParams::param_value(&self) -> f32` return the selected parameter's display name and current value.

### Integration points

1. `BloomRenderer` gains a `pub params: BloomParams` field (initialized to default).
2. `radius_from_tokens` takes `&BloomParams` to read `radius_min`/`radius_max` instead of hardcoded clamp values.
3. `Sphere::pulse_params` and `Sphere::bloom_spread` take `&BloomParams` to read amplitudes and spreads.
4. `apply_gravity` takes `gravity: f32` from params.
5. `apply_repulsion` takes `repulsion_padding: f32` from params.
6. The bloom footer includes the active parameter: `sphere size (min): 12.0 ▸ [/] select  +/- adjust  r reset`

### Key handling in main.rs

The bloom mode arm of the key match forwards `[`, `]`, `+`/`=`, `-`, and `r` to the bloom renderer's params. This requires the bloom renderer to be accessible from the key handler, which it already is via `bloom_renderer: Option<BloomRenderer>`.

## Boundaries

This is purely render-layer state. No changes to `model/` or `events/`. The `BloomParams` struct lives in `render/bloom.rs` alongside the renderer it configures.
