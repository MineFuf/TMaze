use rand::{self, seq::SliceRandom, thread_rng};

const AVAILABLE_PLAYER_CHARS: [char; 8] = ['O', '□', '◇', '☆', '○', '■', '●', '¤'];

pub const GOAL_CHAR: char = '$';

pub fn get_random_player_char() -> char {
    *AVAILABLE_PLAYER_CHARS.choose(&mut thread_rng()).unwrap()
}

pub mod colors {
    pub mod fun {
        use crossterm::style::Color;

        pub fn white() -> Color {
            Color::White
        }

        pub fn red() -> Color {
            Color::Red
        }
    }
}

pub mod paths {
    use std::path::PathBuf;

    #[cfg(not(feature = "local_paths"))]
    pub fn base_path() -> PathBuf {
        use dirs::preference_dir;

        preference_dir().unwrap().join("tmaze")
    }

    #[cfg(feature = "local_paths")]
    pub fn base_path() -> PathBuf {
        PathBuf::from("./")
    }

    pub fn theme_path() -> PathBuf {
        base_path().join("themes/")
    }

    pub fn theme_file_path(theme: &str) -> PathBuf {
        theme_path().join(theme)
    }

    pub fn settings_path() -> PathBuf {
        base_path().join("settings.json5")
    }

    pub fn save_data_path() -> PathBuf {
        base_path().join("data.json")
    }

    pub fn log_file_path() -> PathBuf {
        base_path().join("log.txt")
    }
}
