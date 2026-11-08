//! Śledzenie poziomu szumu otoczenia (minimum statystyczne: szybko w dół, wolno w górę).

use crate::SILENCE_DB;

/// Szacuje szum tła z poziomów kolejnych ramek (dBFS).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoiseFloorTracker {
    floor_db: f32,
    rise_db_per_frame: f32,
    initialized: bool,
}

impl Default for NoiseFloorTracker {
    fn default() -> Self {
        Self::new(10)
    }
}

impl NoiseFloorTracker {
    /// `frame_ms` — długość ramki (wzrost podłogi 3 dB/s, spadek natychmiastowy).
    pub fn new(frame_ms: u32) -> Self {
        Self {
            floor_db: -70.0,
            rise_db_per_frame: 3.0 * frame_ms as f32 / 1000.0,
            initialized: false,
        }
    }

    /// Aktualizuje szacunek poziomem ramki; zwraca bieżący szum (dBFS).
    pub fn update(&mut self, level_db: f32) -> f32 {
        let level = level_db.max(SILENCE_DB);
        if !self.initialized {
            self.floor_db = level;
            self.initialized = true;
        } else if level < self.floor_db {
            self.floor_db = 0.5 * self.floor_db + 0.5 * level;
        } else {
            self.floor_db = (self.floor_db + self.rise_db_per_frame).min(level);
        }
        self.floor_db
    }

    /// Bieżący szum (dBFS).
    pub fn floor_db(&self) -> f32 {
        self.floor_db
    }

    /// Reset (zmiana urządzenia).
    pub fn reset(&mut self) {
        *self = Self {
            rise_db_per_frame: self.rise_db_per_frame,
            ..Self::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_minimum_and_rises_slowly() {
        let mut t = NoiseFloorTracker::new(10);
        assert_eq!(t.update(-50.0), -50.0);
        // Mowa (−20 dB) przez 1 s podnosi podłogę najwyżej o 3 dB.
        for _ in 0..100 {
            t.update(-20.0);
        }
        assert!((t.floor_db() - (-47.0)).abs() < 0.1, "{}", t.floor_db());
        // Cichsze tło — szybki spadek.
        for _ in 0..20 {
            t.update(-65.0);
        }
        assert!(t.floor_db() < -64.0);
        // Długotrwały głośniejszy szum (wentylator) — podłoga dochodzi do niego.
        for _ in 0..1_000 {
            t.update(-40.0);
        }
        assert!((t.floor_db() + 40.0).abs() < 0.01);
        t.reset();
        assert_eq!(t.update(-30.0), -30.0);
        assert_eq!(NoiseFloorTracker::default().floor_db(), -70.0);
    }
}
