use std::collections::{HashMap, HashSet};
use std::time::Instant;

use ratatui::prelude::*;

use crate::model::{Agent, AgentTree, AgentStatus};
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

// --- Tunable parameters ---

const PARAM_COUNT: usize = 8;

/// Tunable parameters for the bloom visualization (sphere sizes, bloom spread, physics).
#[derive(Clone)]
pub(crate) struct BloomParams {
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

    pub(crate) fn nudge(&mut self, up: bool) {
        let idx = self.selected;
        let step = Self::STEPS[idx];
        let val = self.field_mut(idx);
        if up {
            *val = (*val + step).min(Self::MAXS[idx]);
        } else {
            *val = (*val - step).max(Self::MINS[idx]);
        }
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn cycle(&mut self, forward: bool) {
        if forward {
            self.selected = (self.selected + 1) % PARAM_COUNT;
        } else {
            self.selected = (self.selected + PARAM_COUNT - 1) % PARAM_COUNT;
        }
    }

    pub(crate) fn param_name(&self) -> &str {
        Self::NAMES[self.selected]
    }

    pub(crate) fn param_value(&self) -> f32 {
        self.field(self.selected)
    }

    pub(crate) fn bloom_spread_completed(&self) -> f32 {
        self.bloom_spread_idle * 0.625
    }
}

// --- Braille encoding ---

/// Map a 2x4 dot matrix to a Unicode braille character.
/// `dots` is an array of 8 bools: [dot1, dot2, dot3, dot7, dot4, dot5, dot6, dot8]
/// arranged as column-major: left column (rows 0-3), then right column (rows 0-3).
fn braille_char(dots: [bool; 8]) -> char {
    const BIT_MAP: [u32; 8] = [0x01, 0x02, 0x04, 0x40, 0x08, 0x10, 0x20, 0x80];
    let mut code: u32 = 0x2800;
    for (i, &lit) in dots.iter().enumerate() {
        if lit {
            code |= BIT_MAP[i];
        }
    }
    char::from_u32(code).unwrap_or('\u{2800}')
}

// --- Color math ---

fn bloom_falloff(distance_sq: f32, radius: f32, bloom_spread: f32) -> f32 {
    if radius <= 0.0 || bloom_spread <= 0.0 {
        return 0.0;
    }
    (-distance_sq / (radius * radius * bloom_spread)).exp()
}

fn additive_blend(a: (f32, f32, f32), b: (f32, f32, f32)) -> (f32, f32, f32) {
    (
        (a.0 + b.0).min(255.0),
        (a.1 + b.1).min(255.0),
        (a.2 + b.2).min(255.0),
    )
}

fn radius_from_tokens(total_tokens: u64, params: &BloomParams) -> f32 {
    let raw = (total_tokens as f32 / 500.0).sqrt() * 6.0;
    raw.clamp(params.radius_min, params.radius_max)
}

// --- Sphere physics ---

#[derive(Debug, Clone, Copy, PartialEq)]
enum SphereStatus {
    Running,
    Idle,
    Completed,
}

#[derive(Debug)]
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
            base_radius: 12.0,
            pulse_phase: 0.0,
            color,
            status: SphereStatus::Idle,
            fade_start: None,
        }
    }

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

    fn color_multiplier(&self) -> f32 {
        match self.fade_start {
            Some(t) => {
                let elapsed = t.elapsed().as_secs_f32();
                (1.0 - elapsed / 3.0).max(0.0)
            }
            None => 1.0,
        }
    }

    fn is_faded_out(&self) -> bool {
        matches!(self.fade_start, Some(t) if t.elapsed().as_secs_f32() >= 3.0)
    }

    fn min_speed(&self) -> f32 {
        match self.status {
            SphereStatus::Running => 2.0,
            SphereStatus::Idle => 0.5,
            SphereStatus::Completed => 0.0,
        }
    }
}

const EDGE_RESTITUTION: f32 = 0.8;

fn apply_edge_bounce(sphere: &mut Sphere, bounds: (f32, f32), params: &BloomParams) {
    let r = sphere.effective_radius(params);
    if sphere.position.0 < r {
        sphere.position.0 = r;
        sphere.velocity.0 = sphere.velocity.0.abs() * EDGE_RESTITUTION;
    } else if sphere.position.0 > bounds.0 - r {
        sphere.position.0 = bounds.0 - r;
        sphere.velocity.0 = -(sphere.velocity.0.abs() * EDGE_RESTITUTION);
    }
    if sphere.position.1 < r {
        sphere.position.1 = r;
        sphere.velocity.1 = sphere.velocity.1.abs() * EDGE_RESTITUTION;
    } else if sphere.position.1 > bounds.1 - r {
        sphere.position.1 = bounds.1 - r;
        sphere.velocity.1 = -(sphere.velocity.1.abs() * EDGE_RESTITUTION);
    }
}

fn apply_ambient_impulse(sphere: &mut Sphere) {
    let min = sphere.min_speed();
    if min == 0.0 {
        return;
    }
    let speed = (sphere.velocity.0.powi(2) + sphere.velocity.1.powi(2)).sqrt();
    if speed >= min {
        return;
    }
    // Use current direction if moving, otherwise derive from pulse_phase
    let (dx, dy) = if speed > 0.01 {
        (sphere.velocity.0 / speed, sphere.velocity.1 / speed)
    } else {
        (sphere.pulse_phase.cos(), sphere.pulse_phase.sin())
    };
    let boost = (min - speed) * 0.3;
    sphere.velocity.0 += dx * boost;
    sphere.velocity.1 += dy * boost;
}

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

pub(crate) struct BloomRenderer {
    spheres: Vec<Sphere>,
    known_agents: HashSet<String>,
    color_index: usize,
    pixel_buf: Vec<(f32, f32, f32)>,
    buf_width: usize,
    buf_height: usize,
    pub params: BloomParams,
}

impl Default for BloomRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl BloomRenderer {
    pub(crate) fn new() -> Self {
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

    fn sync_spheres(&mut self, agents: &HashMap<&str, &Agent>, center: (f32, f32)) {
        for agent in agents.values() {
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
                self.known_agents.insert(sphere.agent_id.clone());
                self.spheres.push(sphere);
            }
        }

        // Update existing spheres — O(1) lookup via HashMap
        for sphere in &mut self.spheres {
            if let Some(agent) = agents.get(sphere.agent_id.as_str()) {
                let total_tokens = agent.token_usage.as_ref().map_or(0, |t| t.input + t.output);
                sphere.base_radius = radius_from_tokens(total_tokens, &self.params);

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

        // Remove fully faded-out spheres
        self.spheres.retain(|s| !s.is_faded_out());
    }

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

        let bounds = (self.buf_width as f32, self.buf_height as f32);
        for sphere in &mut self.spheres {
            apply_ambient_impulse(sphere);

            sphere.velocity.0 *= 0.9;
            sphere.velocity.1 *= 0.9;
            sphere.position.0 += sphere.velocity.0;
            sphere.position.1 += sphere.velocity.1;

            apply_edge_bounce(sphere, bounds, &self.params);

            let (_, phase_speed) = sphere.pulse_params(&self.params);
            sphere.pulse_phase = (sphere.pulse_phase + phase_speed) % std::f32::consts::TAU;
        }
    }

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
                                let dot_idx = dx * 4 + dy;
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

        let (agents_map, active, total, total_tokens) = match &tree.root {
            Some(root) => {
                let all = root.all_agents();
                let total = all.len();
                let active = all.iter().filter(|a| !matches!(a.status, AgentStatus::Completed)).count();
                let tokens: u64 = all.iter().map(|a| a.token_usage.as_ref().map_or(0, |t| t.input + t.output)).sum();
                let map: HashMap<&str, &Agent> = all.into_iter().map(|a| (a.id.as_str(), a)).collect();
                (Some(map), active, total, tokens)
            }
            None => (None, 0, 0, 0),
        };

        if let Some(agents) = &agents_map {
            self.sync_spheres(agents, center);
        }
        self.simulate(center);
        self.rasterize_and_composite();
        self.encode_to_frame(frame, canvas);

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
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_braille_char_empty() {
        assert_eq!(braille_char([false; 8]), '\u{2800}');
    }

    #[test]
    fn test_braille_char_full() {
        assert_eq!(braille_char([true; 8]), '\u{28FF}');
    }

    #[test]
    fn test_braille_char_single_dots() {
        let mut dots = [false; 8];
        dots[0] = true;
        assert_eq!(braille_char(dots), '\u{2801}');

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
        let p = BloomParams::default();
        assert_eq!(radius_from_tokens(0, &p), 12.0); // min clamp
    }

    #[test]
    fn test_radius_from_tokens_large() {
        let p = BloomParams::default();
        let r = radius_from_tokens(100_000, &p);
        assert!(r <= 45.0, "should clamp to max 45, got {}", r);
        assert!(r >= 44.0, "100k tokens should be near max, got {}", r);
    }

    #[test]
    fn test_radius_from_tokens_mid() {
        let p = BloomParams::default();
        let r = radius_from_tokens(10_000, &p);
        assert!(r > 12.0, "10k tokens should be above min, got {}", r);
        assert!(r < 45.0, "10k tokens should be below max, got {}", r);
    }

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
        // With damped gravity, sphere should be closer to center than it started.
        // It may oscillate past center — that's fine for a spring-like system.
        let dist_from_center = (sphere.position.0 - 50.0).abs();
        assert!(dist_from_center < 50.0, "should be closer to center than start, dist={}", dist_from_center);
    }

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

    #[test]
    fn test_sphere_sync_adds_new_agents() {
        use crate::model::Agent;
        use std::collections::HashMap;
        let mut renderer = BloomRenderer::new();
        let mut root = Agent::new("root".into(), "Main".into());
        root.children.push(Agent::new("c1".into(), "Task 1".into()));
        let all = root.all_agents();
        let agents: HashMap<&str, &Agent> = all.into_iter().map(|a| (a.id.as_str(), a)).collect();

        renderer.sync_spheres(&agents, (100.0, 100.0));

        assert_eq!(renderer.spheres.len(), 2);
        assert!(renderer.known_agents.contains("root"));
        assert!(renderer.known_agents.contains("c1"));
        assert_eq!(renderer.spheres[0].color, PALETTE[0]);
        assert_eq!(renderer.spheres[1].color, PALETTE[1]);
    }

    #[test]
    fn test_sphere_sync_updates_status() {
        use crate::model::{Agent, AgentStatus};
        use std::collections::HashMap;
        let mut renderer = BloomRenderer::new();
        let mut root = Agent::new("root".into(), "Main".into());

        {
            let all = root.all_agents();
            let agents: HashMap<&str, &Agent> = all.into_iter().map(|a| (a.id.as_str(), a)).collect();
            renderer.sync_spheres(&agents, (50.0, 50.0));
        }
        assert_eq!(renderer.spheres[0].status, SphereStatus::Idle);

        root.status = AgentStatus::Running {
            tool_name: "Read".into(),
            key_arg: "file.rs".into(),
        };
        {
            let all = root.all_agents();
            let agents: HashMap<&str, &Agent> = all.into_iter().map(|a| (a.id.as_str(), a)).collect();
            renderer.sync_spheres(&agents, (50.0, 50.0));
        }
        assert_eq!(renderer.spheres[0].status, SphereStatus::Running);
    }

    // --- Sphere method tests ---

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

    #[test]
    fn test_effective_radius_at_zero_phase() {
        let params = BloomParams::default();
        let s = Sphere::new("a".into(), (0.0, 0.0), (0, 0, 0));
        // Idle: amplitude=2.0, sin(0)=0 → base_radius + 0 = 12.0
        assert_eq!(s.effective_radius(&params), 12.0);
    }

    #[test]
    fn test_effective_radius_running_at_peak() {
        let params = BloomParams::default();
        let mut s = Sphere::new("a".into(), (0.0, 0.0), (0, 0, 0));
        s.status = SphereStatus::Running;
        s.pulse_phase = std::f32::consts::FRAC_PI_2; // sin(π/2) = 1.0
        // Running: amplitude=5.0, base=12.0 → 12.0 + 5.0 = 17.0
        assert!((s.effective_radius(&params) - 17.0).abs() < 0.01);
    }

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

    #[test]
    fn test_color_multiplier_no_fade() {
        let s = Sphere::new("a".into(), (0.0, 0.0), (0, 0, 0));
        assert_eq!(s.color_multiplier(), 1.0);
    }

    #[test]
    fn test_color_multiplier_with_fade() {
        let mut s = Sphere::new("a".into(), (0.0, 0.0), (0, 0, 0));
        s.fade_start = Some(std::time::Instant::now());
        // Just started fading — should be close to 1.0
        let mult = s.color_multiplier();
        assert!(mult > 0.9, "just-started fade should be near 1.0, got {}", mult);
        assert!(mult <= 1.0);
    }

    // --- sync_spheres edge cases ---

    #[test]
    fn test_sphere_sync_empty_tree() {
        use crate::model::Agent;
        use std::collections::HashMap;
        let mut renderer = BloomRenderer::new();
        let agents: HashMap<&str, &Agent> = HashMap::new();
        renderer.sync_spheres(&agents, (50.0, 50.0));
        assert_eq!(renderer.spheres.len(), 0);
        assert!(renderer.known_agents.is_empty());
    }

    #[test]
    fn test_sphere_sync_idempotent() {
        use crate::model::Agent;
        use std::collections::HashMap;
        let mut renderer = BloomRenderer::new();
        let root = Agent::new("root".into(), "Main".into());

        {
            let all = root.all_agents();
            let agents: HashMap<&str, &Agent> = all.into_iter().map(|a| (a.id.as_str(), a)).collect();
            renderer.sync_spheres(&agents, (50.0, 50.0));
        }
        assert_eq!(renderer.spheres.len(), 1);

        // Second sync with same tree should not add duplicates
        {
            let all = root.all_agents();
            let agents: HashMap<&str, &Agent> = all.into_iter().map(|a| (a.id.as_str(), a)).collect();
            renderer.sync_spheres(&agents, (50.0, 50.0));
        }
        assert_eq!(renderer.spheres.len(), 1);
    }

    #[test]
    fn test_sphere_sync_sets_fade_on_completion() {
        use crate::model::{Agent, AgentStatus};
        use std::collections::HashMap;
        let mut renderer = BloomRenderer::new();
        let mut root = Agent::new("root".into(), "Main".into());

        {
            let all = root.all_agents();
            let agents: HashMap<&str, &Agent> = all.into_iter().map(|a| (a.id.as_str(), a)).collect();
            renderer.sync_spheres(&agents, (50.0, 50.0));
        }
        assert!(renderer.spheres[0].fade_start.is_none());

        root.status = AgentStatus::Completed;
        {
            let all = root.all_agents();
            let agents: HashMap<&str, &Agent> = all.into_iter().map(|a| (a.id.as_str(), a)).collect();
            renderer.sync_spheres(&agents, (50.0, 50.0));
        }
        assert!(renderer.spheres[0].fade_start.is_some());
        assert_eq!(renderer.spheres[0].status, SphereStatus::Completed);
    }

    #[test]
    fn test_sphere_sync_updates_radius_from_tokens() {
        use crate::model::{Agent, TokenUsage};
        use std::collections::HashMap;
        let mut renderer = BloomRenderer::new();
        let mut root = Agent::new("root".into(), "Main".into());

        {
            let all = root.all_agents();
            let agents: HashMap<&str, &Agent> = all.into_iter().map(|a| (a.id.as_str(), a)).collect();
            renderer.sync_spheres(&agents, (50.0, 50.0));
        }
        assert_eq!(renderer.spheres[0].base_radius, 12.0); // no tokens = min

        root.token_usage = Some(TokenUsage { input: 5000, output: 5000 });
        {
            let all = root.all_agents();
            let agents: HashMap<&str, &Agent> = all.into_iter().map(|a| (a.id.as_str(), a)).collect();
            renderer.sync_spheres(&agents, (50.0, 50.0));
        }
        assert!(renderer.spheres[0].base_radius > 12.0, "should grow with tokens");
    }

    // --- Ambient velocity tests ---

    #[test]
    fn test_min_speed_by_status() {
        let mut s = Sphere::new("a".into(), (50.0, 50.0), (255, 0, 0));

        s.status = SphereStatus::Running;
        assert!(s.min_speed() > 0.0, "running should have nonzero min speed");

        let running_speed = s.min_speed();
        s.status = SphereStatus::Idle;
        assert!(s.min_speed() > 0.0, "idle should have nonzero min speed");
        assert!(s.min_speed() < running_speed, "idle should be slower than running");

        s.status = SphereStatus::Completed;
        assert_eq!(s.min_speed(), 0.0, "completed should have zero min speed");
    }

    #[test]
    fn test_ambient_impulse_boosts_slow_sphere() {
        let mut s = Sphere::new("a".into(), (50.0, 50.0), (255, 0, 0));
        s.status = SphereStatus::Running;
        s.velocity = (0.1, 0.0); // below min_speed
        let speed_before = (s.velocity.0.powi(2) + s.velocity.1.powi(2)).sqrt();
        apply_ambient_impulse(&mut s);
        let speed_after = (s.velocity.0.powi(2) + s.velocity.1.powi(2)).sqrt();
        assert!(speed_after > speed_before, "should boost slow sphere: {} > {}", speed_after, speed_before);
    }

    #[test]
    fn test_ambient_impulse_no_effect_when_fast() {
        let mut s = Sphere::new("a".into(), (50.0, 50.0), (255, 0, 0));
        s.status = SphereStatus::Running;
        s.velocity = (5.0, 5.0); // well above min_speed
        let vel_before = s.velocity;
        apply_ambient_impulse(&mut s);
        assert_eq!(s.velocity, vel_before, "should not change fast sphere");
    }

    #[test]
    fn test_ambient_impulse_no_effect_on_completed() {
        let mut s = Sphere::new("a".into(), (50.0, 50.0), (255, 0, 0));
        s.status = SphereStatus::Completed;
        s.velocity = (0.0, 0.0);
        apply_ambient_impulse(&mut s);
        assert_eq!(s.velocity, (0.0, 0.0), "completed sphere should not get impulse");
    }

    #[test]
    fn test_ambient_impulse_stationary_uses_phase_for_direction() {
        let mut s = Sphere::new("a".into(), (50.0, 50.0), (255, 0, 0));
        s.status = SphereStatus::Running;
        s.velocity = (0.0, 0.0);
        s.pulse_phase = 0.0; // cos(0)=1, sin(0)=0 → impulse in +x direction
        apply_ambient_impulse(&mut s);
        let speed = (s.velocity.0.powi(2) + s.velocity.1.powi(2)).sqrt();
        assert!(speed > 0.0, "stationary sphere should get an impulse");
    }

    // --- Edge bounce tests ---

    #[test]
    fn test_edge_bounce_left_wall() {
        let params = BloomParams::default();
        let mut s = Sphere::new("a".into(), (-5.0, 50.0), (255, 0, 0));
        s.base_radius = 12.0;
        s.velocity = (-3.0, 0.0);
        apply_edge_bounce(&mut s, (200.0, 200.0), &params);
        assert!(s.position.0 >= s.effective_radius(&params));
        assert!(s.velocity.0 > 0.0, "x velocity should be reflected positive");
    }

    #[test]
    fn test_edge_bounce_right_wall() {
        let params = BloomParams::default();
        let mut s = Sphere::new("a".into(), (195.0, 50.0), (255, 0, 0));
        s.base_radius = 12.0;
        s.velocity = (3.0, 0.0);
        apply_edge_bounce(&mut s, (200.0, 200.0), &params);
        assert!(s.position.0 <= 200.0 - s.effective_radius(&params));
        assert!(s.velocity.0 < 0.0, "x velocity should be reflected negative");
    }

    #[test]
    fn test_edge_bounce_top_wall() {
        let params = BloomParams::default();
        let mut s = Sphere::new("a".into(), (50.0, -5.0), (255, 0, 0));
        s.base_radius = 12.0;
        s.velocity = (0.0, -3.0);
        apply_edge_bounce(&mut s, (200.0, 200.0), &params);
        assert!(s.position.1 >= s.effective_radius(&params));
        assert!(s.velocity.1 > 0.0, "y velocity should be reflected positive");
    }

    #[test]
    fn test_edge_bounce_bottom_wall() {
        let params = BloomParams::default();
        let mut s = Sphere::new("a".into(), (50.0, 195.0), (255, 0, 0));
        s.base_radius = 12.0;
        s.velocity = (0.0, 3.0);
        apply_edge_bounce(&mut s, (200.0, 200.0), &params);
        assert!(s.position.1 <= 200.0 - s.effective_radius(&params));
        assert!(s.velocity.1 < 0.0, "y velocity should be reflected negative");
    }

    #[test]
    fn test_edge_bounce_no_effect_when_inside() {
        let params = BloomParams::default();
        let mut s = Sphere::new("a".into(), (100.0, 100.0), (255, 0, 0));
        s.base_radius = 12.0;
        s.velocity = (5.0, -3.0);
        let vel_before = s.velocity;
        apply_edge_bounce(&mut s, (200.0, 200.0), &params);
        assert_eq!(s.velocity, vel_before, "should not change velocity when inside bounds");
    }

    #[test]
    fn test_edge_bounce_restitution_loses_energy() {
        let params = BloomParams::default();
        let mut s = Sphere::new("a".into(), (-5.0, 50.0), (255, 0, 0));
        s.base_radius = 12.0;
        s.velocity = (-10.0, 0.0);
        apply_edge_bounce(&mut s, (200.0, 200.0), &params);
        // Reflected velocity should be less than original due to restitution
        assert!(s.velocity.0 < 10.0, "bounce should lose energy, got {}", s.velocity.0);
        assert!(s.velocity.0 > 0.0, "should still be moving away from wall");
    }

    // --- Physics edge cases ---

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

    #[test]
    fn test_repulsion_zero_distance_no_panic() {
        let params = BloomParams::default();
        let mut a = Sphere::new("a".into(), (50.0, 50.0), (255, 0, 0));
        a.base_radius = 10.0;
        let mut b = Sphere::new("b".into(), (50.0, 50.0), (0, 0, 255));
        b.base_radius = 10.0;

        // Should not panic or produce NaN — guarded by dist_sq > 0.001
        apply_repulsion(&mut a, &mut b, &params);
        assert!(!a.velocity.0.is_nan());
        assert!(!b.velocity.0.is_nan());
    }

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

}
