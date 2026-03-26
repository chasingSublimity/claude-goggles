# Bloom Renderer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a braille-based bloom visualization mode that renders agents as pulsing, glowing spheres, activated via `--viz bloom`.

**Architecture:** A new `BloomRenderer` in `src/render/bloom.rs` implements the existing `Renderer` trait (changed to `&mut self`). It maintains internal simulation state (sphere positions, velocities, phases) and renders to a braille pixel buffer each frame. `main.rs` holds both renderers and switches between them via a `VizMode` enum.

**Tech Stack:** Rust, ratatui (Frame/Span/Line/Color), crossterm, std::f32 math, Unicode braille (U+2800..U+28FF)

**Spec:** `docs/superpowers/specs/2026-03-25-bloom-renderer-design.md`

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `src/render/bloom.rs` | Create | BloomRenderer, Sphere, physics, rasterization, braille encoding, compositing |
| `src/render/mod.rs` | Modify | Add `pub mod bloom;`, change trait to `&mut self` |
| `src/render/tree_view.rs` | Modify | Update `impl Renderer` signature to `&mut self` |
| `src/model/mod.rs` | Modify | Add `Agent::all_agents()` flat traversal method |
| `src/main.rs` | Modify | Add `--viz` flag, VizMode enum, hold both renderers, `v` toggle, key suppression |

---

### Task 1: Add `Agent::all_agents()` to model

**Files:**
- Modify: `src/model/mod.rs:36-63` (impl Agent block)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `src/model/mod.rs`:

```rust
#[test]
fn test_all_agents_flat_traversal() {
    let mut root = Agent::new("root".into(), "Main".into());
    let mut child = Agent::new("c1".into(), "Task 1".into());
    child.children.push(Agent::new("c1a".into(), "Subtask".into()));
    root.children.push(child);
    root.children.push(Agent::new("c2".into(), "Task 2".into()));

    let all = root.all_agents();
    let ids: Vec<&str> = all.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, vec!["root", "c1", "c1a", "c2"]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test model::tests::test_all_agents_flat_traversal`
Expected: FAIL — `all_agents` method does not exist

- [ ] **Step 3: Write minimal implementation**

Add to the `impl Agent` block in `src/model/mod.rs`, after `find_mut`:

```rust
/// Collect all agents in the tree as a flat list (depth-first).
pub fn all_agents(&self) -> Vec<&Agent> {
    let mut result = vec![self];
    for child in &self.children {
        result.extend(child.all_agents());
    }
    result
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test model::tests::test_all_agents_flat_traversal`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/model/mod.rs
git commit -m "feat(model): add Agent::all_agents() flat traversal"
```

---

### Task 2: Change Renderer trait to `&mut self`

**Files:**
- Modify: `src/render/mod.rs:6-8`
- Modify: `src/render/tree_view.rs:9-10`
- Modify: `src/main.rs:82` (render call site)

- [ ] **Step 1: Update trait signature**

In `src/render/mod.rs`, change line 7:
```rust
fn render(&mut self, tree: &AgentTree, frame: &mut Frame, scroll_offset: usize, selected: usize);
```

- [ ] **Step 2: Update TreeViewRenderer impl**

In `src/render/tree_view.rs`, change line 10:
```rust
fn render(&mut self, tree: &AgentTree, frame: &mut Frame, scroll_offset: usize, selected: usize) {
```

- [ ] **Step 3: Update main.rs call site**

In `src/main.rs`, change line 67 from `let renderer = TreeViewRenderer;` to `let mut renderer = TreeViewRenderer;`.

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All 63 tests pass. No logic changes, just signature widening.

- [ ] **Step 5: Commit**

```bash
git add src/render/mod.rs src/render/tree_view.rs src/main.rs
git commit -m "refactor(render): change Renderer trait to &mut self for stateful renderers"
```

---

### Task 3: Bloom core — braille encoding and color math

**Files:**
- Create: `src/render/bloom.rs` (partial — pure functions only)
- Modify: `src/render/mod.rs` (add `pub mod bloom;`)

This task builds the testable pure functions: braille encoding, bloom falloff, additive blending, radius-from-tokens.

- [ ] **Step 1: Write failing tests**

Create `src/render/bloom.rs` with tests only:

```rust
use std::collections::HashSet;
use std::time::Instant;

use ratatui::prelude::*;

use crate::model::{AgentTree, AgentStatus};
use super::Renderer;

// --- Constants ---

const PALETTE: [(u8, u8, u8); 8] = [
    (0, 210, 255),     // Cyan
    (255, 105, 180),   // Magenta
    (255, 217, 61),    // Gold
    (107, 203, 119),   // Green
    (255, 107, 53),    // Coral
    (180, 130, 255),   // Lavender
    (0, 200, 170),     // Teal
    (255, 150, 150),   // Rose
];

const INTENSITY_THRESHOLD: f32 = 0.05;

// --- Braille encoding ---

/// Map a 2x4 dot matrix to a Unicode braille character.
/// `dots` is an array of 8 bools: [dot1, dot2, dot3, dot7, dot4, dot5, dot6, dot8]
/// arranged as column-major: left column (rows 0-3), then right column (rows 0-3).
fn braille_char(dots: [bool; 8]) -> char {
    todo!()
}

// --- Color math ---

fn bloom_falloff(distance_sq: f32, radius: f32, bloom_spread: f32) -> f32 {
    todo!()
}

fn additive_blend(a: (f32, f32, f32), b: (f32, f32, f32)) -> (f32, f32, f32) {
    todo!()
}

fn radius_from_tokens(total_tokens: u64) -> f32 {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_braille_char_empty() {
        // No dots lit = blank braille
        assert_eq!(braille_char([false; 8]), '\u{2800}');
    }

    #[test]
    fn test_braille_char_full() {
        // All dots lit = full braille block
        assert_eq!(braille_char([true; 8]), '\u{28FF}');
    }

    #[test]
    fn test_braille_char_single_dots() {
        // Dot 1 only (top-left) = bit 0x01
        let mut dots = [false; 8];
        dots[0] = true;
        assert_eq!(braille_char(dots), '\u{2801}');

        // Dot 8 only (bottom-right) = bit 0x80
        let mut dots = [false; 8];
        dots[7] = true;
        assert_eq!(braille_char(dots), '\u{2880}');
    }

    #[test]
    fn test_bloom_falloff_at_center() {
        let intensity = bloom_falloff(0.0, 10.0, 0.8);
        assert!((intensity - 1.0).abs() < 0.01, "center should be ~1.0, got {}", intensity);
    }

    #[test]
    fn test_bloom_falloff_at_edge() {
        // At distance = radius, with bloom_spread = 0.8
        let radius = 10.0;
        let distance_sq = radius * radius;
        let intensity = bloom_falloff(distance_sq, radius, 0.8);
        assert!(intensity < 0.4, "edge should be dim, got {}", intensity);
        assert!(intensity > 0.0, "edge should not be zero, got {}", intensity);
    }

    #[test]
    fn test_bloom_falloff_far_away() {
        let intensity = bloom_falloff(10000.0, 5.0, 0.5);
        assert!(intensity < 0.001, "far away should be negligible, got {}", intensity);
    }

    #[test]
    fn test_additive_blend_no_overflow() {
        let a = (100.0, 150.0, 200.0);
        let b = (50.0, 80.0, 30.0);
        let result = additive_blend(a, b);
        assert_eq!(result, (150.0, 230.0, 230.0));
    }

    #[test]
    fn test_additive_blend_clamps_at_255() {
        let a = (200.0, 200.0, 200.0);
        let b = (200.0, 100.0, 200.0);
        let result = additive_blend(a, b);
        assert_eq!(result, (255.0, 255.0, 255.0));
    }

    #[test]
    fn test_radius_from_tokens_zero() {
        assert_eq!(radius_from_tokens(0), 3.0); // min clamp
    }

    #[test]
    fn test_radius_from_tokens_large() {
        let r = radius_from_tokens(100_000);
        assert!(r <= 20.0, "should clamp to max 20, got {}", r);
        assert!(r >= 19.0, "100k tokens should be near max, got {}", r);
    }

    #[test]
    fn test_radius_from_tokens_mid() {
        let r = radius_from_tokens(10_000);
        assert!(r > 3.0, "10k tokens should be above min, got {}", r);
        assert!(r < 20.0, "10k tokens should be below max, got {}", r);
    }
}
```

- [ ] **Step 2: Add module declaration**

In `src/render/mod.rs`, add `pub mod bloom;` after `pub mod tree_view;`.

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test render::bloom`
Expected: FAIL — all functions are `todo!()`

- [ ] **Step 4: Implement the pure functions**

Replace the `todo!()` bodies in `src/render/bloom.rs`:

```rust
fn braille_char(dots: [bool; 8]) -> char {
    // Bit positions: dots[0..4] = left column, dots[4..8] = right column
    // Mapping: dot1=0x01, dot2=0x02, dot3=0x04, dot7=0x40, dot4=0x08, dot5=0x10, dot6=0x20, dot8=0x80
    const BIT_MAP: [u32; 8] = [0x01, 0x02, 0x04, 0x40, 0x08, 0x10, 0x20, 0x80];
    let mut code: u32 = 0x2800;
    for (i, &lit) in dots.iter().enumerate() {
        if lit {
            code |= BIT_MAP[i];
        }
    }
    char::from_u32(code).unwrap_or('\u{2800}')
}

fn bloom_falloff(distance_sq: f32, radius: f32, bloom_spread: f32) -> f32 {
    (-distance_sq / (radius * radius * bloom_spread)).exp()
}

fn additive_blend(a: (f32, f32, f32), b: (f32, f32, f32)) -> (f32, f32, f32) {
    (
        (a.0 + b.0).min(255.0),
        (a.1 + b.1).min(255.0),
        (a.2 + b.2).min(255.0),
    )
}

fn radius_from_tokens(total_tokens: u64) -> f32 {
    let raw = (total_tokens as f32 / 500.0).sqrt() * 4.0;
    raw.clamp(3.0, 20.0)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test render::bloom`
Expected: All 10 tests pass

- [ ] **Step 6: Commit**

```bash
git add src/render/bloom.rs src/render/mod.rs
git commit -m "feat(bloom): add braille encoding and color math with tests"
```

---

### Task 4: Bloom core — Sphere struct and physics simulation

**Files:**
- Modify: `src/render/bloom.rs`

- [ ] **Step 1: Write failing physics tests**

Add to the test module in `src/render/bloom.rs`:

```rust
#[test]
fn test_gravity_pulls_toward_center() {
    let center = (50.0, 50.0);
    let mut sphere = Sphere::new("a".into(), (100.0, 50.0), (0, 210, 255));
    // Run 20 ticks of gravity
    for _ in 0..20 {
        apply_gravity(&mut sphere, center);
        sphere.position.0 += sphere.velocity.0;
        sphere.position.1 += sphere.velocity.1;
        sphere.velocity.0 *= 0.9;
        sphere.velocity.1 *= 0.9;
    }
    assert!(sphere.position.0 < 100.0, "should move toward center, x={}", sphere.position.0);
    assert!(sphere.position.0 > 50.0, "should not overshoot center, x={}", sphere.position.0);
}

#[test]
fn test_repulsion_separates_spheres() {
    let mut a = Sphere::new("a".into(), (50.0, 50.0), (255, 0, 0));
    a.base_radius = 10.0;
    let mut b = Sphere::new("b".into(), (52.0, 50.0), (0, 0, 255));
    b.base_radius = 10.0;
    let initial_dist = (a.position.0 - b.position.0).abs();

    for _ in 0..20 {
        apply_repulsion(&mut a, &mut b);
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

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test render::bloom`
Expected: FAIL — `Sphere`, `apply_gravity`, `apply_repulsion` don't exist

- [ ] **Step 3: Implement Sphere and physics**

Add to `src/render/bloom.rs` above the tests module:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum SphereStatus {
    Running,
    Idle,
    Completed,
}

struct Sphere {
    agent_id: String,
    position: (f32, f32),
    velocity: (f32, f32),
    base_radius: f32,
    pulse_phase: f32,
    color: (u8, u8, u8),
    status: SphereStatus,
    fade_start: Option<Instant>,
}

impl Sphere {
    fn new(agent_id: String, position: (f32, f32), color: (u8, u8, u8)) -> Self {
        Self {
            agent_id,
            position,
            velocity: (0.0, 0.0),
            base_radius: 3.0,
            pulse_phase: 0.0,
            color,
            status: SphereStatus::Idle,
            fade_start: None,
        }
    }

    fn effective_radius(&self) -> f32 {
        let (amplitude, _) = self.pulse_params();
        self.base_radius + amplitude * self.pulse_phase.sin()
    }

    fn pulse_params(&self) -> (f32, f32) {
        match self.status {
            SphereStatus::Running => (3.0, 0.15),
            SphereStatus::Idle => (1.0, 0.05),
            SphereStatus::Completed => (0.0, 0.0),
        }
    }

    fn bloom_spread(&self) -> f32 {
        match self.status {
            SphereStatus::Running => 0.8,
            SphereStatus::Idle => 0.5,
            SphereStatus::Completed => 0.3,
        }
    }

    fn color_multiplier(&self) -> f32 {
        match self.fade_start {
            Some(t) => {
                let elapsed = t.elapsed().as_secs_f32();
                (1.0 - elapsed / 3.0).max(0.2)
            }
            None => 1.0,
        }
    }
}

fn apply_gravity(sphere: &mut Sphere, center: (f32, f32)) {
    let dx = center.0 - sphere.position.0;
    let dy = center.1 - sphere.position.1;
    sphere.velocity.0 += dx * 0.02;
    sphere.velocity.1 += dy * 0.02;
}

fn apply_repulsion(a: &mut Sphere, b: &mut Sphere) {
    let dx = b.position.0 - a.position.0;
    let dy = b.position.1 - a.position.1;
    let dist_sq = dx * dx + dy * dy;
    let min_dist = a.effective_radius() + b.effective_radius();
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

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test render::bloom`
Expected: All 12 tests pass

- [ ] **Step 5: Commit**

```bash
git add src/render/bloom.rs
git commit -m "feat(bloom): add Sphere struct and physics simulation"
```

---

### Task 5: Bloom core — pixel buffer, rasterization, and rendering

**Files:**
- Modify: `src/render/bloom.rs`

This task adds `BloomRenderer`, the pixel buffer, rasterization pipeline, and the `Renderer` trait implementation. Depends on tasks 1-4.

- [ ] **Step 1: Write sphere sync test**

Add to the test module in `src/render/bloom.rs`:

```rust
#[test]
fn test_sphere_sync_adds_new_agents() {
    use crate::model::Agent;
    let mut renderer = BloomRenderer::new();
    let mut tree = AgentTree::new();
    let mut root = Agent::new("root".into(), "Main".into());
    root.children.push(Agent::new("c1".into(), "Task 1".into()));
    tree.root = Some(root);

    renderer.sync_spheres(&tree, (100.0, 100.0));

    assert_eq!(renderer.spheres.len(), 2);
    assert!(renderer.known_agents.contains("root"));
    assert!(renderer.known_agents.contains("c1"));
    // Root gets palette index 0 (cyan)
    assert_eq!(renderer.spheres[0].color, PALETTE[0]);
    assert_eq!(renderer.spheres[1].color, PALETTE[1]);
}

#[test]
fn test_sphere_sync_updates_status() {
    use crate::model::{Agent, AgentStatus};
    let mut renderer = BloomRenderer::new();
    let mut tree = AgentTree::new();
    tree.root = Some(Agent::new("root".into(), "Main".into()));

    renderer.sync_spheres(&tree, (50.0, 50.0));
    assert_eq!(renderer.spheres[0].status, SphereStatus::Idle);

    // Change agent status
    tree.root.as_mut().unwrap().status = AgentStatus::Running {
        tool_name: "Read".into(),
        key_arg: "file.rs".into(),
    };
    renderer.sync_spheres(&tree, (50.0, 50.0));
    assert_eq!(renderer.spheres[0].status, SphereStatus::Running);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test render::bloom`
Expected: FAIL — `BloomRenderer` doesn't exist

- [ ] **Step 3: Implement BloomRenderer**

Add to `src/render/bloom.rs`, above the Sphere definition:

```rust
pub struct BloomRenderer {
    spheres: Vec<Sphere>,
    known_agents: HashSet<String>,
    color_index: usize,
    pixel_buf: Vec<(f32, f32, f32)>,
    buf_width: usize,
    buf_height: usize,
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
        }
    }

    fn sync_spheres(&mut self, tree: &AgentTree, center: (f32, f32)) {
        let agents = match &tree.root {
            Some(root) => root.all_agents(),
            None => return,
        };

        for agent in &agents {
            if !self.known_agents.contains(&agent.id) {
                let color = PALETTE[self.color_index % PALETTE.len()];
                self.color_index += 1;
                let offset_x = (self.spheres.len() as f32 * 7.0) % 20.0 - 10.0;
                let offset_y = (self.spheres.len() as f32 * 11.0) % 20.0 - 10.0;
                let mut sphere = Sphere::new(
                    agent.id.clone(),
                    (center.0 + offset_x, center.1 + offset_y),
                    color,
                );
                sphere.velocity = (offset_x * 0.1, offset_y * 0.1);
                self.spheres.push(sphere);
                self.known_agents.insert(agent.id.clone());
            }
        }

        // Update existing spheres
        for sphere in &mut self.spheres {
            if let Some(agent) = agents.iter().find(|a| a.id == sphere.agent_id) {
                let total_tokens = agent.token_usage.as_ref().map_or(0, |t| t.input + t.output);
                sphere.base_radius = radius_from_tokens(total_tokens);

                let new_status = match &agent.status {
                    AgentStatus::Running { .. } => SphereStatus::Running,
                    AgentStatus::Idle => SphereStatus::Idle,
                    AgentStatus::Completed => SphereStatus::Completed,
                };

                if new_status == SphereStatus::Completed && sphere.status != SphereStatus::Completed {
                    sphere.fade_start = Some(Instant::now());
                }
                sphere.status = new_status;
            }
        }
    }

    fn simulate(&mut self, center: (f32, f32)) {
        // Gravity
        for sphere in &mut self.spheres {
            apply_gravity(sphere, center);
        }

        // Pairwise repulsion (safe index-based to avoid double borrow)
        let len = self.spheres.len();
        for i in 0..len {
            for j in (i + 1)..len {
                let (left, right) = self.spheres.split_at_mut(j);
                apply_repulsion(&mut left[i], &mut right[0]);
            }
        }

        // Damping + pulse advance
        for sphere in &mut self.spheres {
            sphere.velocity.0 *= 0.9;
            sphere.velocity.1 *= 0.9;
            sphere.position.0 += sphere.velocity.0;
            sphere.position.1 += sphere.velocity.1;

            let (_, phase_speed) = sphere.pulse_params();
            sphere.pulse_phase = (sphere.pulse_phase + phase_speed) % std::f32::consts::TAU;
        }
    }

    fn rasterize_and_composite(&mut self) {
        // Clear buffer
        for pixel in &mut self.pixel_buf {
            *pixel = (0.0, 0.0, 0.0);
        }

        for sphere in &self.spheres {
            let radius = sphere.effective_radius();
            let spread = sphere.bloom_spread();
            let mult = sphere.color_multiplier();
            let (cr, cg, cb) = sphere.color;
            let color = (cr as f32 * mult, cg as f32 * mult, cb as f32 * mult);

            let r_ceil = (radius * 2.0).ceil() as i32; // sample beyond radius for bloom
            let cx = sphere.position.0 as i32;
            let cy = sphere.position.1 as i32;

            for dy in -r_ceil..=r_ceil {
                for dx in -r_ceil..=r_ceil {
                    let px = cx + dx;
                    let py = cy + dy;
                    if px < 0 || py < 0 || px >= self.buf_width as i32 || py >= self.buf_height as i32 {
                        continue;
                    }
                    let dist_sq = (dx * dx + dy * dy) as f32;
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

    fn encode_to_frame(&self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = Vec::new();
        let term_w = area.width as usize;
        let term_h = area.height as usize;

        for row in 0..term_h {
            let mut spans: Vec<Span> = Vec::new();
            for col in 0..term_w {
                let mut dots = [false; 8];
                let mut max_intensity: f32 = 0.0;
                let mut max_color = (0.0_f32, 0.0_f32, 0.0_f32);

                for dy in 0..4 {
                    for dx in 0..2 {
                        let px = col * 2 + dx;
                        let py = row * 4 + dy;
                        if px < self.buf_width && py < self.buf_height {
                            let idx = py * self.buf_width + px;
                            let (r, g, b) = self.pixel_buf[idx];
                            let intensity = r + g + b;
                            if intensity > INTENSITY_THRESHOLD * 255.0 {
                                let dot_idx = dx * 4 + dy; // column-major: left col [0..4], right col [4..8]
                                dots[dot_idx] = true;
                                if intensity > max_intensity {
                                    max_intensity = intensity;
                                    max_color = (r, g, b);
                                }
                            }
                        }
                    }
                }

                let ch = braille_char(dots);
                let fg = Color::Rgb(
                    max_color.0.min(255.0) as u8,
                    max_color.1.min(255.0) as u8,
                    max_color.2.min(255.0) as u8,
                );
                spans.push(Span::styled(ch.to_string(), Style::default().fg(fg)));
            }
            lines.push(Line::from(spans));
        }

        let widget = ratatui::widgets::Paragraph::new(lines);
        frame.render_widget(widget, area);
    }
}

impl Renderer for BloomRenderer {
    fn render(&mut self, tree: &AgentTree, frame: &mut Frame, _scroll_offset: usize, _selected: usize) {
        let area = frame.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        let canvas = chunks[0];
        let new_w = canvas.width as usize * 2;
        let new_h = canvas.height as usize * 4;
        if new_w != self.buf_width || new_h != self.buf_height {
            self.buf_width = new_w;
            self.buf_height = new_h;
            self.pixel_buf = vec![(0.0, 0.0, 0.0); new_w * new_h];
        }

        let center = (new_w as f32 / 2.0, new_h as f32 / 2.0);
        self.sync_spheres(tree, center);
        self.simulate(center);
        self.rasterize_and_composite();
        self.encode_to_frame(frame, canvas);

        // Footer (reuse tree_view helpers via direct computation)
        let (active, total, total_tokens) = count_and_tokens(tree);
        let token_str = if total_tokens >= 1000 {
            format!("{:.1}k tok", total_tokens as f64 / 1000.0)
        } else {
            format!("{} tok", total_tokens)
        };
        let footer = Line::from(vec![
            Span::styled(
                format!("agents: {} ({} active)", total, active),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(token_str, Style::default().fg(Color::DarkGray)),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("dropped: {}", tree.dropped_events),
                Style::default().fg(if tree.dropped_events > 0 { Color::Yellow } else { Color::DarkGray }),
            ),
            Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
            Span::styled("q: quit  v: tree view", Style::default().fg(Color::DarkGray)),
        ]);
        frame.render_widget(ratatui::widgets::Paragraph::new(footer), chunks[1]);
    }
}

fn count_and_tokens(tree: &AgentTree) -> (usize, usize, u64) {
    match &tree.root {
        None => (0, 0, 0),
        Some(root) => {
            let agents = root.all_agents();
            let total = agents.len();
            let active = agents.iter().filter(|a| !matches!(a.status, AgentStatus::Completed)).count();
            let tokens: u64 = agents.iter().map(|a| {
                a.token_usage.as_ref().map_or(0, |t| t.input + t.output)
            }).sum();
            (active, total, tokens)
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test render::bloom`
Expected: All 14 tests pass

- [ ] **Step 5: Verify full build**

Run: `cargo build`
Expected: Clean compile (possibly dead_code warnings, which are fine during development)

- [ ] **Step 6: Commit**

```bash
git add src/render/bloom.rs
git commit -m "feat(bloom): implement BloomRenderer with pixel buffer and rasterization"
```

---

### Task 6: Wire up CLI flag and renderer toggle in main.rs

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Update Cli struct with --viz flag**

In `src/main.rs`, change the `Cli` struct:

```rust
#[derive(Parser)]
#[command(name = "claude-goggles", about = "Visualize Claude Code agent activity")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    /// Visualization mode: tree or bloom
    #[arg(long, default_value = "tree")]
    viz: String,
}
```

- [ ] **Step 2: Add VizMode enum and update run_tui**

Add above `fn main()`:

```rust
#[derive(Clone, Copy, PartialEq)]
enum VizMode {
    Tree,
    Bloom,
}
```

Change `run_tui()` to accept the viz mode. Update `main()`:

```rust
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Init) => cli::init()?,
        Some(Commands::Clean) => cli::clean()?,
        None => {
            let viz = match cli.viz.as_str() {
                "bloom" => VizMode::Bloom,
                _ => VizMode::Tree,
            };
            run_tui(viz)?;
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Update run_tui to hold both renderers and handle v toggle**

Replace the entire `run_tui` function:

```rust
fn run_tui(initial_mode: VizMode) -> anyhow::Result<()> {
    let sock_path = cli::socket_dir()?.join("goggles.sock");

    let rt = tokio::runtime::Runtime::new()?;
    let (tx, mut rx) = mpsc::channel(1000);

    let listener = SocketListener::new(sock_path);
    rt.spawn(async move {
        if let Err(e) = listener.listen(tx).await {
            eprintln!("Socket error: {}", e);
        }
    });

    enable_raw_mode()?;
    std::io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut tree_renderer = TreeViewRenderer;
    let mut bloom_renderer = render::bloom::BloomRenderer::new();
    let mut viz_mode = initial_mode;
    let mut tree = AgentTree::new();
    let mut scroll_offset: usize = 0;
    let mut selected: usize = 0;

    loop {
        while let Ok(ev) = rx.try_recv() {
            apply_event(&mut tree, ev);
        }

        let visible_count = tree.visible_agent_count();

        terminal.draw(|frame| {
            match viz_mode {
                VizMode::Tree => tree_renderer.render(&tree, frame, scroll_offset, selected),
                VizMode::Bloom => bloom_renderer.render(&tree, frame, scroll_offset, selected),
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char('v') => {
                        viz_mode = match viz_mode {
                            VizMode::Tree => VizMode::Bloom,
                            VizMode::Bloom => VizMode::Tree,
                        };
                    }
                    _ if viz_mode == VizMode::Tree => match key.code {
                        KeyCode::Up | KeyCode::Char('k') => {
                            selected = selected.saturating_sub(1);
                            let selected_line = selected * 2 + 1;
                            if selected_line < scroll_offset {
                                scroll_offset = selected_line;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if visible_count > 0 {
                                selected = (selected + 1).min(visible_count - 1);
                            }
                            let selected_line = selected * 2 + 1;
                            let area_height = terminal.size()?.height.saturating_sub(2) as usize;
                            if selected_line + 2 > scroll_offset + area_height {
                                scroll_offset = (selected_line + 2).saturating_sub(area_height);
                            }
                        }
                        KeyCode::Char('c') => {
                            if let Some(agent) = tree.nth_visible_agent_mut(selected) {
                                agent.collapsed = !agent.collapsed;
                            }
                        }
                        _ => {}
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    std::io::stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}
```

- [ ] **Step 4: Update imports**

Remove the unused `use render::tree_view::TreeViewRenderer;` import at the top and add it locally, or keep it — either works since it's still used. Ensure the `Renderer` import stays. No new crate dependencies needed.

- [ ] **Step 5: Build and test**

Run: `cargo build && cargo test`
Expected: Clean build, all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire up --viz bloom flag with renderer toggle via v key"
```

---

### Task 7: Manual smoke test and visual tuning

**Files:** None (testing only)

- [ ] **Step 1: Run with tree mode (default)**

Run: `cargo run` in one terminal, run `claude-goggles init` first if hooks not installed. Start a Claude Code session in another terminal. Verify tree view works as before.

- [ ] **Step 2: Run with bloom mode**

Run: `cargo run -- --viz bloom`
Verify: Dark screen appears with footer. When Claude Code starts, colored braille spheres appear and pulse.

- [ ] **Step 3: Test v toggle**

While running in bloom mode, press `v`. Verify it switches to tree view. Press `v` again — bloom view should restore with spheres in their previous positions.

- [ ] **Step 4: Test keyboard suppression**

In bloom mode, press `j`, `k`, `c`. Verify no visible effect. Switch to tree mode with `v`, verify j/k/c work normally.

- [ ] **Step 5: Commit any tuning adjustments**

If physics constants or colors need adjustment based on visual testing, commit those changes:

```bash
git add -p src/render/bloom.rs
git commit -m "fix(bloom): tune physics constants after visual testing"
```
