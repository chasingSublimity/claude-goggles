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
}
