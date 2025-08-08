use std::sync::LazyLock;
use std::sync::Mutex;

use anyhow::Result;

static DIRLOCK: LazyLock<Mutex<u8>> = LazyLock::new(|| Mutex::new(0));

pub fn set_dir<F>(dir: &std::path::Path, clos: F) -> Result<()>
where
    F: Fn() -> Result<()>,
{
    let _handle = DIRLOCK.lock().unwrap();

    std::env::set_current_dir(dir)?;

    clos()?;

    Ok(())
}
