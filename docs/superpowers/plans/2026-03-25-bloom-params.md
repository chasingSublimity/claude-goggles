# Bloom Parameter Tuning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make bloom renderer visual parameters adjustable in real-time via keyboard controls.

**Architecture:** A `BloomParams` struct holds all tunable values with defaults, step sizes, and clamping ranges. It lives on `BloomRenderer` as a public field. All hardcoded constants in physics/rendering functions are replaced with reads from `BloomParams`. Key events in bloom mode are forwarded from `main.rs` to the params.

**Tech Stack:** Rust, ratatui, crossterm

---

### Task 1: BloomParams struct and unit tests

**Files:**
- Modify: `src/render/bloom.rs`

- [ ] **Step 1: Write failing tests for BloomParams**

Add these tests inside the existing `#[cfg(test)] mod tests` block at the bottom of `src/render/bloom.rs`:

```rust
    // --- BloomParams tests ---

    #[test]
    fn test_bloom_params_default_values() {
        let p = BloomParams::default();
        assert_eq!(p.radius_min, 12.0);
        assert_eq!(p.radius_max, 45.0);
        assert_eq!(p.bloom_spread_running, 1.2);
        assert_eq!(p.bloom_spread_idle, 0.8);
        assert_eq!(p.pulse_amp_running, 5.0);
        assert_eq!(p.pulse_amp_idle, 2.0);
        assert_eq!(p.gravity, 0.02);
        assert_eq!(p.repulsion_padding, 10.0);
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn test_bloom_params_nudge_up() {
        let mut p = BloomParams::default();
        p.selected = 0; // radius_min, step=2.0
        p.nudge(true);
        assert_eq!(p.radius_min, 14.0);
    }

    #[test]
    fn test_bloom_params_nudge_down() {
        let mut p = BloomParams::default();
        p.selected = 0; // radius_min, step=2.0
        p.nudge(false);
        assert_eq!(p.radius_min, 10.0);
    }

    #[test]
    fn test_bloom_params_nudge_clamps_min() {
        let mut p = BloomParams::default();
        p.selected = 6; // gravity, default=0.02, step=0.005, min=0.0
        // Nudge down many times — should clamp at 0.0
        for _ in 0..20 {
            p.nudge(false);
        }
        assert!(p.gravity >= 0.0);
    }

    #[test]
    fn test_bloom_params_nudge_clamps_max() {
        let mut p = BloomParams::default();
        p.selected = 6; // gravity, max=0.1
        for _ in 0..100 {
            p.nudge(true);
        }
        assert!(p.gravity <= 0.1);
    }

    #[test]
    fn test_bloom_params_reset() {
        let mut p = BloomParams::default();
        p.radius_min = 99.0;
        p.gravity = 0.09;
        p.selected = 5;
        p.reset();
        assert_eq!(p.radius_min, 12.0);
        assert_eq!(p.gravity, 0.02);
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn test_bloom_params_cycle_wraps() {
        let mut p = BloomParams::default();
        p.selected = 7; // last param
        p.cycle(true);
        assert_eq!(p.selected, 0);

        p.selected = 0;
        p.cycle(false);
        assert_eq!(p.selected, 7);
    }

    #[test]
    fn test_bloom_params_param_name() {
        let p = BloomParams::default();
        assert_eq!(p.param_name(), "sphere size (min)");
    }

    #[test]
    fn test_bloom_params_param_value() {
        let p = BloomParams::default();
        assert_eq!(p.param_value(), 12.0);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test bloom`
Expected: compilation errors — `BloomParams` does not exist yet.

- [ ] **Step 3: Implement BloomParams**

Add the following struct and impl block in `src/render/bloom.rs`, just after the `INTENSITY_THRESHOLD` constant and before the `// --- Braille encoding ---` section:

```rust
// --- Tunable parameters ---

const PARAM_COUNT: usize = 8;

#[derive(Clone)]
pub struct BloomParams {
    pub radius_min: f32,
    pub radius_max: f32,
    pub bloom_spread_running: f32,
    pub bloom_spread_idle: f32,
    pub pulse_amp_running: f32,
    pub pulse_amp_idle: f32,
    pub gravity: f32,
    pub repulsion_padding: f32,
    pub selected: usize,
}

impl Default for BloomParams {
    fn default() -> Self {
        Self {
            radius_min: 12.0,
            radius_max: 45.0,
            bloom_spread_running: 1.2,
            bloom_spread_idle: 0.8,
            pulse_amp_running: 5.0,
            pulse_amp_idle: 2.0,
            gravity: 0.02,
            repulsion_padding: 10.0,
            selected: 0,
        }
    }
}

impl BloomParams {
    /// Step sizes for each parameter, indexed by `selected`.
    const STEPS: [f32; PARAM_COUNT] = [2.0, 2.0, 0.1, 0.1, 0.5, 0.5, 0.005, 2.0];
    const MINS: [f32; PARAM_COUNT] = [4.0, 4.0, 0.1, 0.1, 0.0, 0.0, 0.0, 0.0];
    const MAXS: [f32; PARAM_COUNT] = [60.0, 80.0, 3.0, 3.0, 15.0, 15.0, 0.1, 40.0];
    const NAMES: [&str; PARAM_COUNT] = [
        "sphere size (min)",
        "sphere size (max)",
        "bloom spread (run)",
        "bloom spread (idle)",
        "pulse amp (run)",
        "pulse amp (idle)",
        "gravity",
        "repulsion padding",
    ];

    fn field_mut(&mut self, index: usize) -> &mut f32 {
        match index {
            0 => &mut self.radius_min,
            1 => &mut self.radius_max,
            2 => &mut self.bloom_spread_running,
            3 => &mut self.bloom_spread_idle,
            4 => &mut self.pulse_amp_running,
            5 => &mut self.pulse_amp_idle,
            6 => &mut self.gravity,
            7 => &mut self.repulsion_padding,
            _ => unreachable!(),
        }
    }

    fn field(&self, index: usize) -> f32 {
        match index {
            0 => self.radius_min,
            1 => self.radius_max,
            2 => self.bloom_spread_running,
            3 => self.bloom_spread_idle,
            4 => self.pulse_amp_running,
            5 => self.pulse_amp_idle,
            6 => self.gravity,
            7 => self.repulsion_padding,
            _ => unreachable!(),
        }
    }

    pub fn nudge(&mut self, up: bool) {
        let step = Self::STEPS[self.selected];
        let val = self.field_mut(self.selected);
        if up {
            *val = (*val + step).min(Self::MAXS[self.selected]);
        } else {
            *val = (*val - step).max(Self::MINS[self.selected]);
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn cycle(&mut self, forward: bool) {
        if forward {
            self.selected = (self.selected + 1) % PARAM_COUNT;
        } else {
            self.selected = (self.selected + PARAM_COUNT - 1) % PARAM_COUNT;
        }
    }

    pub fn param_name(&self) -> &str {
        Self::NAMES[self.selected]
    }

    pub fn param_value(&self) -> f32 {
        self.field(self.selected)
    }

    pub fn bloom_spread_completed(&self) -> f32 {
        self.bloom_spread_idle * 0.625
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test bloom`
Expected: All `test_bloom_params_*` tests pass. Some existing tests (`test_radius_from_tokens_*`, `test_gravity_pulls_toward_center`, `test_repulsion_*`, `test_sphere_sync_*`, etc.) still pass since we haven't changed any signatures yet.

- [ ] **Step 5: Commit**

```bash
git add src/render/bloom.rs
git commit -m "feat(bloom): add BloomParams struct with nudge/reset/cycle"
```

---

### Task 2: Thread BloomParams through renderer internals

**Files:**
- Modify: `src/render/bloom.rs`

This task replaces hardcoded constants with reads from `BloomParams`. We change function signatures and update all call sites. Existing tests that rely on old signatures or old default values are updated.

- [ ] **Step 1: Update `radius_from_tokens` to take params**

Replace the existing `radius_from_tokens` function:

```rust
fn radius_from_tokens(total_tokens: u64, params: &BloomParams) -> f32 {
    let raw = (total_tokens as f32 / 500.0).sqrt() * 6.0;
    raw.clamp(params.radius_min, params.radius_max)
}
```

- [ ] **Step 2: Update `Sphere` methods to take `&BloomParams`**

Replace `pulse_params` and `bloom_spread` on `Sphere`:

```rust
    fn effective_radius(&self, params: &BloomParams) -> f32 {
        let (amplitude, _) = self.pulse_params(params);
        self.base_radius + amplitude * self.pulse_phase.sin()
    }

    fn pulse_params(&self, params: &BloomParams) -> (f32, f32) {
        match self.status {
            SphereStatus::Running => (params.pulse_amp_running, 0.15),
            SphereStatus::Idle => (params.pulse_amp_idle, 0.05),
            SphereStatus::Completed => (0.0, 0.0),
        }
    }

    fn bloom_spread(&self, params: &BloomParams) -> f32 {
        match self.status {
            SphereStatus::Running => params.bloom_spread_running,
            SphereStatus::Idle => params.bloom_spread_idle,
            SphereStatus::Completed => params.bloom_spread_completed(),
        }
    }
```

- [ ] **Step 3: Update `apply_gravity` and `apply_repulsion` to take params**

```rust
fn apply_gravity(sphere: &mut Sphere, center: (f32, f32), gravity: f32) {
    let dx = center.0 - sphere.position.0;
    let dy = center.1 - sphere.position.1;
    sphere.velocity.0 += dx * gravity;
    sphere.velocity.1 += dy * gravity;
}

fn apply_repulsion(a: &mut Sphere, b: &mut Sphere, params: &BloomParams) {
    let dx = b.position.0 - a.position.0;
    let dy = b.position.1 - a.position.1;
    let dist_sq = dx * dx + dy * dy;
    let min_dist = a.effective_radius(params) + b.effective_radius(params) + params.repulsion_padding;
    let min_dist_sq = min_dist * min_dist;

    if dist_sq < min_dist_sq && dist_sq > 0.001 {
        let dist = dist_sq.sqrt();
        let overlap = min_dist - dist;
        let nx = dx / dist;
        let ny = dy / dist;
        let force = overlap * 0.5;
        a.velocity.0 -= nx * force;
        a.velocity.1 -= ny * force;
        b.velocity.0 += nx * force;
        b.velocity.1 += ny * force;
    }
}
```

- [ ] **Step 4: Add `pub params: BloomParams` to `BloomRenderer` and update all internal call sites**

Add the field to the struct:

```rust
pub struct BloomRenderer {
    spheres: Vec<Sphere>,
    known_agents: HashSet<String>,
    color_index: usize,
    pixel_buf: Vec<(f32, f32, f32)>,
    buf_width: usize,
    buf_height: usize,
    pub params: BloomParams,
}
```

Update `Default` impl and `new()`:

```rust
impl Default for BloomRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl BloomRenderer {
    pub fn new() -> Self {
        Self {
            spheres: Vec::new(),
            known_agents: HashSet::new(),
            color_index: 0,
            pixel_buf: Vec::new(),
            buf_width: 0,
            buf_height: 0,
            params: BloomParams::default(),
        }
    }
```

Update `sync_spheres` — the line that calls `radius_from_tokens`:

```rust
                sphere.base_radius = radius_from_tokens(total_tokens, &self.params);
```

Update `simulate` — gravity and repulsion calls:

```rust
    fn simulate(&mut self, center: (f32, f32)) {
        let gravity = self.params.gravity;
        for sphere in &mut self.spheres {
            apply_gravity(sphere, center, gravity);
        }

        let len = self.spheres.len();
        for i in 0..len {
            for j in (i + 1)..len {
                let (left, right) = self.spheres.split_at_mut(j);
                apply_repulsion(&mut left[i], &mut right[0], &self.params);
            }
        }

        for sphere in &mut self.spheres {
            sphere.velocity.0 *= 0.9;
            sphere.velocity.1 *= 0.9;
            sphere.position.0 += sphere.velocity.0;
            sphere.position.1 += sphere.velocity.1;

            let (_, phase_speed) = sphere.pulse_params(&self.params);
            sphere.pulse_phase = (sphere.pulse_phase + phase_speed) % std::f32::consts::TAU;
        }
    }
```

Update `rasterize_and_composite` — clone params to avoid borrow conflict with `self.spheres`/`self.pixel_buf`:

```rust
    fn rasterize_and_composite(&mut self) {
        self.pixel_buf.fill((0.0, 0.0, 0.0));
        let params = self.params.clone();

        for sphere in &self.spheres {
            let radius = sphere.effective_radius(&params);
            let spread = sphere.bloom_spread(&params);
            let mult = sphere.color_multiplier();
            let (cr, cg, cb) = sphere.color;
            let color = (f32::from(cr) * mult, f32::from(cg) * mult, f32::from(cb) * mult);

            let r_ceil = (radius * 2.0).ceil() as i32;
            let cx = sphere.position.0;
            let cy = sphere.position.1;
            let cx_i = cx as i32;
            let cy_i = cy as i32;

            for dy in -r_ceil..=r_ceil {
                for dx in -r_ceil..=r_ceil {
                    let px = cx_i + dx;
                    let py = cy_i + dy;
                    if px < 0 || py < 0 || px >= self.buf_width as i32 || py >= self.buf_height as i32 {
                        continue;
                    }
                    let dist_sq = (px as f32 - cx).powi(2) + (py as f32 - cy).powi(2);
                    let intensity = bloom_falloff(dist_sq, radius, spread);
                    if intensity < INTENSITY_THRESHOLD {
                        continue;
                    }
                    let idx = py as usize * self.buf_width + px as usize;
                    let contribution = (color.0 * intensity, color.1 * intensity, color.2 * intensity);
                    self.pixel_buf[idx] = additive_blend(self.pixel_buf[idx], contribution);
                }
            }
        }
    }
```

- [ ] **Step 5: Update existing tests that use changed signatures**

Update `test_radius_from_tokens_zero`:
```rust
    #[test]
    fn test_radius_from_tokens_zero() {
        let p = BloomParams::default();
        assert_eq!(radius_from_tokens(0, &p), 12.0); // min clamp
    }
```

Update `test_radius_from_tokens_large`:
```rust
    #[test]
    fn test_radius_from_tokens_large() {
        let p = BloomParams::default();
        let r = radius_from_tokens(100_000, &p);
        assert!(r <= 45.0, "should clamp to max 45, got {}", r);
        assert!(r >= 44.0, "100k tokens should be near max, got {}", r);
    }
```

Update `test_radius_from_tokens_mid`:
```rust
    #[test]
    fn test_radius_from_tokens_mid() {
        let p = BloomParams::default();
        let r = radius_from_tokens(10_000, &p);
        assert!(r > 12.0, "10k tokens should be above min, got {}", r);
        assert!(r < 45.0, "10k tokens should be below max, got {}", r);
    }
```

Update `test_gravity_pulls_toward_center`:
```rust
    #[test]
    fn test_gravity_pulls_toward_center() {
        let center = (50.0, 50.0);
        let mut sphere = Sphere::new("a".into(), (100.0, 50.0), (0, 210, 255));
        for _ in 0..20 {
            apply_gravity(&mut sphere, center, 0.02);
            sphere.position.0 += sphere.velocity.0;
            sphere.position.1 += sphere.velocity.1;
            sphere.velocity.0 *= 0.9;
            sphere.velocity.1 *= 0.9;
        }
        let dist_from_center = (sphere.position.0 - 50.0).abs();
        assert!(dist_from_center < 50.0, "should be closer to center than start, dist={}", dist_from_center);
    }
```

Update `test_repulsion_separates_spheres`:
```rust
    #[test]
    fn test_repulsion_separates_spheres() {
        let params = BloomParams::default();
        let mut a = Sphere::new("a".into(), (50.0, 50.0), (255, 0, 0));
        a.base_radius = 10.0;
        let mut b = Sphere::new("b".into(), (52.0, 50.0), (0, 0, 255));
        b.base_radius = 10.0;
        let initial_dist = (a.position.0 - b.position.0).abs();

        for _ in 0..20 {
            apply_repulsion(&mut a, &mut b, &params);
            a.position.0 += a.velocity.0;
            a.position.1 += a.velocity.1;
            b.position.0 += b.velocity.0;
            b.position.1 += b.velocity.1;
            a.velocity.0 *= 0.9;
            a.velocity.1 *= 0.9;
            b.velocity.0 *= 0.9;
            b.velocity.1 *= 0.9;
        }
        let final_dist = (a.position.0 - b.position.0).abs();
        assert!(final_dist > initial_dist, "spheres should separate: {} > {}", final_dist, initial_dist);
    }
```

Update `test_repulsion_no_effect_when_far_apart`:
```rust
    #[test]
    fn test_repulsion_no_effect_when_far_apart() {
        let params = BloomParams::default();
        let mut a = Sphere::new("a".into(), (0.0, 0.0), (255, 0, 0));
        a.base_radius = 5.0;
        let mut b = Sphere::new("b".into(), (100.0, 0.0), (0, 0, 255));
        b.base_radius = 5.0;

        apply_repulsion(&mut a, &mut b, &params);
        assert_eq!(a.velocity, (0.0, 0.0));
        assert_eq!(b.velocity, (0.0, 0.0));
    }
```

Update `test_repulsion_zero_distance_no_panic`:
```rust
    #[test]
    fn test_repulsion_zero_distance_no_panic() {
        let params = BloomParams::default();
        let mut a = Sphere::new("a".into(), (50.0, 50.0), (255, 0, 0));
        a.base_radius = 10.0;
        let mut b = Sphere::new("b".into(), (50.0, 50.0), (0, 0, 255));
        b.base_radius = 10.0;

        apply_repulsion(&mut a, &mut b, &params);
        assert!(!a.velocity.0.is_nan());
        assert!(!b.velocity.0.is_nan());
    }
```

Update `test_effective_radius_at_zero_phase`:
```rust
    #[test]
    fn test_effective_radius_at_zero_phase() {
        let params = BloomParams::default();
        let s = Sphere::new("a".into(), (0.0, 0.0), (0, 0, 0));
        assert_eq!(s.effective_radius(&params), 12.0);
    }
```

Update `test_effective_radius_running_at_peak`:
```rust
    #[test]
    fn test_effective_radius_running_at_peak() {
        let params = BloomParams::default();
        let mut s = Sphere::new("a".into(), (0.0, 0.0), (0, 0, 0));
        s.status = SphereStatus::Running;
        s.pulse_phase = std::f32::consts::FRAC_PI_2;
        // Running: amplitude=5.0, base=12.0 → 12.0 + 5.0 = 17.0
        assert!((s.effective_radius(&params) - 17.0).abs() < 0.01);
    }
```

Update `test_pulse_params_by_status`:
```rust
    #[test]
    fn test_pulse_params_by_status() {
        let params = BloomParams::default();
        let mut s = Sphere::new("a".into(), (0.0, 0.0), (0, 0, 0));

        s.status = SphereStatus::Running;
        assert_eq!(s.pulse_params(&params), (5.0, 0.15));

        s.status = SphereStatus::Idle;
        assert_eq!(s.pulse_params(&params), (2.0, 0.05));

        s.status = SphereStatus::Completed;
        assert_eq!(s.pulse_params(&params), (0.0, 0.0));
    }
```

Update `test_bloom_spread_by_status`:
```rust
    #[test]
    fn test_bloom_spread_by_status() {
        let params = BloomParams::default();
        let mut s = Sphere::new("a".into(), (0.0, 0.0), (0, 0, 0));

        s.status = SphereStatus::Running;
        assert_eq!(s.bloom_spread(&params), 1.2);

        s.status = SphereStatus::Idle;
        assert_eq!(s.bloom_spread(&params), 0.8);

        s.status = SphereStatus::Completed;
        assert_eq!(s.bloom_spread(&params), 0.5);
    }
```

Update `test_sphere_new_defaults` — `base_radius` changed from 6.0 to 12.0:
```rust
    #[test]
    fn test_sphere_new_defaults() {
        let s = Sphere::new("test".into(), (10.0, 20.0), (255, 0, 0));
        assert_eq!(s.agent_id, "test");
        assert_eq!(s.position, (10.0, 20.0));
        assert_eq!(s.velocity, (0.0, 0.0));
        assert_eq!(s.base_radius, 12.0);
        assert_eq!(s.pulse_phase, 0.0);
        assert_eq!(s.color, (255, 0, 0));
        assert_eq!(s.status, SphereStatus::Idle);
        assert!(s.fade_start.is_none());
    }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test bloom`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/render/bloom.rs
git commit -m "refactor(bloom): thread BloomParams through renderer internals"
```

---

### Task 3: Update bloom footer to show active parameter

**Files:**
- Modify: `src/render/bloom.rs`

- [ ] **Step 1: Update the footer in the `Renderer` impl**

In the `impl Renderer for BloomRenderer` block, replace the footer section (the `let footer = Line::from(vec![...])` block and the `frame.render_widget` call for the footer) with:

```rust
        // Footer
        let token_str = super::footer::format_tokens(total_tokens);
        let param_display = format!(
            "{}: {:.3}",
            self.params.param_name(),
            self.params.param_value()
        );
        let footer = Line::from(vec![
            Span::styled(param_display, Style::default().fg(Color::Cyan)),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("agents: {} ({} active)", total, active),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(token_str, Style::default().fg(Color::DarkGray)),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[/] select  +/- adjust  r reset  v tree",
                Style::default().fg(Color::DarkGray),
            ),
        ]);
        frame.render_widget(ratatui::widgets::Paragraph::new(footer), chunks[1]);
```

- [ ] **Step 2: Build to verify**

Run: `cargo build`
Expected: Compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add src/render/bloom.rs
git commit -m "feat(bloom): show active parameter in footer"
```

---

### Task 4: Wire up key handling in main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add bloom-mode key handling**

In `src/main.rs`, the key match currently has two arms after `Char('v')`:

```rust
                    _ if viz_mode == VizMode::Tree => match key.code {
                        // ... tree controls
                    }
                    _ => {}
```

Add a bloom-mode arm between the tree arm and the final `_ => {}`. Replace the `_ => {}` at the end with:

```rust
                    _ if viz_mode == VizMode::Bloom => {
                        if let Some(ref mut bloom) = bloom_renderer {
                            match key.code {
                                KeyCode::Char('[') => bloom.params.cycle(false),
                                KeyCode::Char(']') => bloom.params.cycle(true),
                                KeyCode::Char('+') | KeyCode::Char('=') => bloom.params.nudge(true),
                                KeyCode::Char('-') => bloom.params.nudge(false),
                                KeyCode::Char('r') => bloom.params.reset(),
                                _ => {}
                            }
                        }
                    }
                    _ => {}
```

- [ ] **Step 2: Build to verify**

Run: `cargo build`
Expected: Compiles cleanly.

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat(bloom): wire up [/] +/- r keys for parameter tuning"
```
