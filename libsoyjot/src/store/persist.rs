use std::env;
use std::path::Path;

use super::error::StoreError;

// Default hard-coded storage directory.
const DIR: &'static str = "./drop";

pub fn assert_dir(conf_dir: Option<String>) {
    let dir = match conf_dir {
        Some(d) if !d.is_empty() => d,
        _ => DIR.to_string(),
    };

    let create_dir = |d| {
        std::fs::create_dir(d).expect("failed to create storage directory");
    };

    match dir_exists(&dir) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => create_dir(dir),
        Ok(false) => create_dir(dir),

        Err(err) => {
            panic!("bad directory: {}", err.to_string());
        }

        _ => {}
    }
}

pub fn write_clipboard_file<S>(name: S, content: &[u8]) -> Result<(), StoreError>
where
    S: AsRef<Path>,
{
    let path = Path::new(DIR).join(name.as_ref());
    std::fs::write(path, content)?;

    Ok(())
}

pub fn read_clipboard_file<S>(id: S) -> Result<Vec<u8>, StoreError>
where
    S: AsRef<Path>,
{
    let path = Path::new(DIR).join(id.as_ref());
    let data = std::fs::read(path)?;

    Ok(data)
}

pub fn rm_clipboard_file<S>(id: S) -> Result<(), StoreError>
where
    S: AsRef<Path>,
{
    let path = Path::new(DIR).join(id.as_ref());
    std::fs::remove_file(path)?;

    Ok(())
}

pub fn dir_exists(dst: &str) -> std::io::Result<bool> {
    let mut pwd = env::current_dir()?;
    pwd.push(dst);
    let metadata = std::fs::metadata(pwd)?;

    Ok(metadata.is_dir())
}
