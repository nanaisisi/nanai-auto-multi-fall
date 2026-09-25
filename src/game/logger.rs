use bevy::prelude::*;

/// プレイログをテキストファイルに随時記録するリソース
#[derive(Resource)]
pub struct GameLogger {
    pub file: Option<std::fs::File>,
    pub start_time: std::time::Instant,
}

impl Default for GameLogger {
    fn default() -> Self {
        let base_dir = if std::path::Path::new("Cargo.toml").exists() {
            std::path::PathBuf::from("logs")
        } else if std::path::Path::new("../Cargo.toml").exists() {
            std::path::PathBuf::from("../logs")
        } else {
            std::path::PathBuf::from("logs")
        };

        let _ = std::fs::create_dir_all(&base_dir);
        let timestamp = chrono_like_timestamp();
        let path = base_dir.join(format!("game_{}.log", timestamp));
        let file = std::fs::File::create(&path).ok();
        let mut logger = Self {
            file,
            start_time: std::time::Instant::now(),
        };
        logger.log(&format!(
            "=== NANAI AUTO MULTI-FALL LOG SESSION STARTED [{}] ===",
            timestamp
        ));
        logger
    }
}

impl GameLogger {
    pub fn log(&mut self, message: &str) {
        if let Some(file) = &mut self.file {
            use std::io::Write;
            let elapsed = self.start_time.elapsed().as_secs_f32();
            let _ = writeln!(file, "[{:>8.2}s] {}", elapsed, message);
            let _ = file.flush();
        }
    }

    pub fn log_raw(&mut self, text: &str) {
        if let Some(file) = &mut self.file {
            use std::io::Write;
            let _ = write!(file, "{}", text);
            let _ = file.flush();
        }
    }
}

fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", secs)
}
