use crate::consts::VIRUS_BASE_SPEED;

pub fn virus_count_for_level(level: u32) -> u32 {
    1 + level / 2
}

pub fn virus_speed_for_level(level: u32) -> f32 {
    VIRUS_BASE_SPEED + (level as f32 - 1.0) * 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_1_has_one_virus() {
        assert_eq!(virus_count_for_level(1), 1);
    }

    #[test]
    fn level_4_has_three_viruses() {
        assert_eq!(virus_count_for_level(4), 3);
    }

    #[test]
    fn speed_grows_linearly() {
        assert_eq!(virus_speed_for_level(1), VIRUS_BASE_SPEED);
        assert_eq!(virus_speed_for_level(2), VIRUS_BASE_SPEED + 10.0);
    }
}
