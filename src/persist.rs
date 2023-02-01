use std::env;
use std::path::Path;

// Default hard-coded storage directory.
pub const DIR: &str = "drop";

pub fn assert_dir() {
    let create_dir = |dir| {
        std::fs::create_dir(dir).expect("failed to create storage directory");
    };

    match check_dir(DIR) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => create_dir(DIR),
        Ok(false) => create_dir(DIR),

        Err(err) => {
            panic!("bad directory: {}", err.to_string());
        }

        _ => {}
    }
}

pub fn write_clipboard_file<S>(name: S, content: &[u8]) -> std::io::Result<()>
where
    S: AsRef<Path>,
{
    let path = Path::new(DIR).join(name.as_ref());
    std::fs::write(path, content)
}

pub fn read_clipboard_file<S>(id: S) -> std::io::Result<Vec<u8>>
where
    S: AsRef<Path>,
{
    let path = Path::new(DIR).join(id.as_ref());
    std::fs::read(path)
}

pub fn check_dir(dst: &str) -> std::io::Result<bool> {
    let mut pwd = env::current_dir()?;
    pwd.push(dst);
    let metadata = std::fs::metadata(pwd)?;

    Ok(metadata.is_dir())
}
