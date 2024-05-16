use std::env;
use std::path::Path;

use tokio::fs;

use super::error::StoreError;

// Default hard-coded storage directory.
const DIR: &'static str = "./drop";

pub async fn assert_dir(conf_dir: Option<String>) {
    let dir = match conf_dir {
        Some(s) if !s.is_empty() => s,
        _ => DIR.to_string(),
    };

    let result = match dir_exists(&dir).await {
        Ok(false) => create_dir(&dir).await,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => create_dir(&dir).await,
        Err(err) => {
            panic!("bad directory: {}", err.to_string());
        }
        _ => Ok(()),
    };

    result.expect("failed to create store directory '{dir}'");
}

async fn create_dir<S>(dir: S) -> Result<(), StoreError>
where
    S: AsRef<Path>,
{
    fs::create_dir(dir).await?;
    Ok(())
}

pub async fn write_clipboard_file<S>(name: S, content: &[u8]) -> Result<(), StoreError>
where
    S: AsRef<Path>,
{
    let path = Path::new(DIR).join(name.as_ref());
    fs::write(path, content).await?;

    Ok(())
}

pub async fn read_clipboard_file<S>(id: S) -> Result<Vec<u8>, StoreError>
where
    S: AsRef<Path>,
{
    let path = Path::new(DIR).join(id.as_ref());
    let data = fs::read(path).await?;

    Ok(data)
}

pub async fn rm_clipboard_file<S>(id: S) -> Result<(), StoreError>
where
    S: AsRef<Path>,
{
    let path = Path::new(DIR).join(id.as_ref());
    fs::remove_file(path).await?;

    Ok(())
}

pub async fn dir_exists(dst: &str) -> std::io::Result<bool> {
    let mut pwd = env::current_dir()?;
    pwd.push(dst);

    let metadata = fs::metadata(pwd).await?;
    Ok(metadata.is_dir())
}
