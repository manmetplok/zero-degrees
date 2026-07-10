use shared::Urgency;

pub fn base_points(urgency: Urgency) -> i32 {
    match urgency {
        Urgency::Critical => 40,
        Urgency::High => 30,
        Urgency::Normal => 20,
        Urgency::Low => 10,
    }
}

pub fn elapsed_seconds(at: i64, since: i64) -> i64 {
    (at - since).max(0)
}

pub fn is_burning(elapsed_seconds: i64, target_seconds: i64) -> bool {
    elapsed_seconds > target_seconds
}

pub struct ClearScore {
    pub points_awarded: i32,
    pub speed_bonus_awarded: i32,
    pub burning: bool,
}

/// Score a clear: burning clears get half the base points and no bonus,
/// on-time clears get full base points plus a bonus that grows the earlier
/// the reply lands inside the target window.
pub fn score_clear(urgency: Urgency, response_seconds: i64, target_seconds: i64) -> ClearScore {
    let base = base_points(urgency);
    if is_burning(response_seconds, target_seconds) {
        return ClearScore {
            points_awarded: (base / 2).max(1),
            speed_bonus_awarded: 0,
            burning: true,
        };
    }
    let remaining = (target_seconds - response_seconds).max(0) as f64;
    let speed_ratio = if target_seconds > 0 {
        (remaining / target_seconds as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let speed_bonus_awarded = (base as f64 * speed_ratio * 0.5).round() as i32;
    ClearScore {
        points_awarded: base + speed_bonus_awarded,
        speed_bonus_awarded,
        burning: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_is_not_burning_within_its_target() {
        assert!(!is_burning(299, 300));
        assert!(!is_burning(300, 300));
    }

    #[test]
    fn message_starts_burning_once_it_exceeds_its_target() {
        assert!(is_burning(301, 300));
    }

    #[test]
    fn instant_clear_awards_full_points_and_max_speed_bonus() {
        let score = score_clear(Urgency::Normal, 0, 3600);
        assert!(!score.burning);
        assert_eq!(score.points_awarded, base_points(Urgency::Normal) + 10);
        assert_eq!(score.speed_bonus_awarded, 10);
    }

    #[test]
    fn clear_right_at_the_target_awards_full_points_with_no_bonus() {
        let score = score_clear(Urgency::High, 900, 900);
        assert!(!score.burning);
        assert_eq!(score.points_awarded, base_points(Urgency::High));
        assert_eq!(score.speed_bonus_awarded, 0);
    }

    #[test]
    fn burning_clear_awards_half_points_and_no_speed_bonus() {
        let score = score_clear(Urgency::Critical, 301, 300);
        assert!(score.burning);
        assert_eq!(score.points_awarded, base_points(Urgency::Critical) / 2);
        assert_eq!(score.speed_bonus_awarded, 0);
    }

    #[test]
    fn extremely_overdue_clear_still_awards_partial_points() {
        let score = score_clear(Urgency::Low, 99_999, 1);
        assert!(score.burning);
        assert_eq!(score.points_awarded, base_points(Urgency::Low) / 2);
    }

    #[test]
    fn on_time_clear_always_scores_higher_than_burning_clear_of_same_urgency() {
        let on_time = score_clear(Urgency::High, 100, 900);
        let burning = score_clear(Urgency::High, 901, 900);
        assert!(on_time.points_awarded > burning.points_awarded);
    }
}
