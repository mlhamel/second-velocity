use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const SCREENSHOTS: [&str; 4] = [
    "01-flight-earth.png",
    "02-combat.png",
    "03-docked-trade.png",
    "04-chief-encounter.png",
];

static CAPTURE_LOCK: Mutex<()> = Mutex::new(());
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(180);

/// Runs a separate, rendered client; the MCP simulation remains headless.
/// Publish only after the complete tour succeeds, never from old captures.
pub fn capture_readme_screenshots(root: &Path, client: &Path) -> Result<Vec<String>, String> {
    let _guard = CAPTURE_LOCK
        .try_lock()
        .map_err(|_| "a screenshot capture is already running".to_owned())?;
    if !client.is_file() {
        return Err(
            "build evr-client alongside evr-mcp first: cargo build -p evr-client -p evr-mcp".into(),
        );
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let staging = root
        .join("target")
        .join(format!("readme-capture-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&staging).map_err(|err| err.to_string())?;
    let log_path = staging.join("client.log");
    let log = fs::File::create(&log_path).map_err(|err| err.to_string())?;
    let mut child = Command::new(client)
        .current_dir(root)
        .arg("--capture-tour")
        .arg("--capture-dir")
        .arg(&staging)
        .stdin(Stdio::null())
        // Never let desktop logs corrupt the MCP JSON-RPC stdout stream.
        .stdout(log.try_clone().map_err(|err| err.to_string())?)
        .stderr(log)
        .spawn()
        .map_err(|err| format!("could not launch desktop client: {err}"))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(status)) => {
                return Err(format!(
                    "capture tour exited with {status}; log: {}",
                    log_path.display()
                ));
            }
            Ok(None) if start.elapsed() < CAPTURE_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(100))
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "capture tour timed out or could not be monitored ({result:?}); log: {}",
                    log_path.display()
                ));
            }
        }
    }
    publish_captures(&staging, &root.join("docs/screenshots"))
        .map_err(|err| format!("{err}; log: {}", log_path.display()))
}

fn publish_captures(staging: &Path, destination: &Path) -> Result<Vec<String>, String> {
    // Validate every new file before overwriting any existing README image.
    let mut images = Vec::new();
    for name in SCREENSHOTS {
        let bytes =
            fs::read(staging.join(name)).map_err(|err| format!("missing capture {name}: {err}"))?;
        if png_dimensions(&bytes).is_none() {
            return Err(format!("capture {name} has no valid PNG header/dimensions"));
        }
        images.push((name, bytes));
    }
    fs::create_dir_all(destination).map_err(|err| err.to_string())?;
    for (name, bytes) in images {
        fs::write(destination.join(name), bytes).map_err(|err| err.to_string())?;
    }
    Ok(SCREENSHOTS
        .iter()
        .map(|name| format!("docs/screenshots/{name}"))
        .collect())
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 33 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    (width > 0 && height > 0).then_some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = include_bytes!("../../../docs/screenshots/01-flight-earth.png");

    #[test]
    fn screenshot_headers_require_png_and_nonzero_dimensions() {
        assert!(png_dimensions(PNG).is_some());
        assert!(png_dimensions(b"not a PNG").is_none());
        let mut invalid = PNG.to_vec();
        invalid[16..20].copy_from_slice(&0_u32.to_be_bytes());
        assert!(png_dimensions(&invalid).is_none());
    }

    #[test]
    fn incomplete_tour_does_not_replace_existing_images() {
        let root = std::env::temp_dir().join(format!("evr-capture-publish-{}", std::process::id()));
        let staging = root.join("staging");
        let destination = root.join("published");
        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join(SCREENSHOTS[0]), b"previous capture").unwrap();
        fs::write(staging.join(SCREENSHOTS[0]), PNG).unwrap();
        assert!(publish_captures(&staging, &destination).is_err());
        assert_eq!(
            fs::read(destination.join(SCREENSHOTS[0])).unwrap(),
            b"previous capture"
        );
        for name in SCREENSHOTS {
            fs::write(staging.join(name), PNG).unwrap();
        }
        fs::write(staging.join(SCREENSHOTS[3]), b"invalid PNG").unwrap();
        assert!(publish_captures(&staging, &destination).is_err());
        assert_eq!(
            fs::read(destination.join(SCREENSHOTS[0])).unwrap(),
            b"previous capture"
        );
        fs::write(staging.join(SCREENSHOTS[3]), PNG).unwrap();
        assert_eq!(publish_captures(&staging, &destination).unwrap().len(), 4);
        assert_eq!(fs::read(destination.join(SCREENSHOTS[0])).unwrap(), PNG);
        fs::remove_dir_all(root).unwrap();
    }
}
