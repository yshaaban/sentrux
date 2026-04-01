use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn temp_root(prefix: &str, label: &str, subdirs: &[&str]) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("{prefix}-{label}-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(&root).expect("create temp root");

    for subdir in subdirs {
        std::fs::create_dir_all(root.join(subdir)).expect("create temp subdir");
    }

    root
}
