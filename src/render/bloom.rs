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

// --- Sphere physics ---

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
    sphere.velocity.0 += dx * 0.005;
    sphere.velocity.1 += dy * 0.005;
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
        assert_eq!(radius_from_tokens(0), 3.0);
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

    #[test]
    fn test_gravity_pulls_toward_center() {
        let center = (50.0, 50.0);
        let mut sphere = Sphere::new("a".into(), (100.0, 50.0), (0, 210, 255));
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
}
