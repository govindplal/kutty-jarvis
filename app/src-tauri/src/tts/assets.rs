//! The Piper engine and voice: pinned downloads, checksum verification and
//! safe extraction. Neither is bundled (ADR 0014): Piper's 2023 build links
//! GPL-3.0 espeak-ng, so it is fetched on first use and run as a separate
//! process.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

/// A pinned download.
pub struct Asset {
    pub url: &'static str,
    pub sha256: &'static str,
}

/// `rhasspy/piper-voices` revision the voice files are pinned to.
#[cfg(test)]
const VOICES_REV: &str = "5ec43b186ef915a195d3ddbcfd9916e76c503c3f";

pub const VOICE_NAME: &str = "en_US-ljspeech-medium";

#[cfg(windows)]
pub const ENGINE: Option<Asset> = Some(Asset {
    url: "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip",
    sha256: "f3c58906402b24f3a96d92145f58acba6d86c9b5db896d207f78dc80811efcea",
});
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const ENGINE: Option<Asset> = Some(Asset {
    url: "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_linux_x86_64.tar.gz",
    sha256: "a50cb45f355b7af1f6d758c1b360717877ba0a398cc8cbe6d2a7a3a26e225992",
});
#[cfg(not(any(windows, all(target_os = "linux", target_arch = "x86_64"))))]
pub const ENGINE: Option<Asset> = None;

pub const VOICE_MODEL: Asset = Asset {
    url: "https://huggingface.co/rhasspy/piper-voices/resolve/5ec43b186ef915a195d3ddbcfd9916e76c503c3f/en/en_US/ljspeech/medium/en_US-ljspeech-medium.onnx",
    sha256: "6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22",
};
pub const VOICE_CONFIG: Asset = Asset {
    url: "https://huggingface.co/rhasspy/piper-voices/resolve/5ec43b186ef915a195d3ddbcfd9916e76c503c3f/en/en_US/ljspeech/medium/en_US-ljspeech-medium.onnx.json",
    sha256: "141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860",
};
/// Kept next to the voice, as its licence asks.
pub const VOICE_CARD: Asset = Asset {
    url: "https://huggingface.co/rhasspy/piper-voices/resolve/5ec43b186ef915a195d3ddbcfd9916e76c503c3f/en/en_US/ljspeech/medium/MODEL_CARD",
    sha256: "fbee1529c89d36b3fe76d7e9f3f832dce17f44900a52d76a9bda735654766b4d",
};

/// Where an installed engine and voice live.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PiperPaths {
    pub executable: PathBuf,
    pub model: PathBuf,
    pub config: PathBuf,
}

impl PiperPaths {
    pub fn under(root: &Path) -> Self {
        let exe = if cfg!(windows) { "piper.exe" } else { "piper" };
        Self {
            executable: root.join("engine").join("piper").join(exe),
            model: root.join("voices").join(format!("{VOICE_NAME}.onnx")),
            config: root.join("voices").join(format!("{VOICE_NAME}.onnx.json")),
        }
    }

    pub fn is_installed(&self) -> bool {
        self.executable.is_file() && self.model.is_file() && self.config.is_file()
    }
}

pub fn piper_root(app_local_data: &Path) -> PathBuf {
    app_local_data.join("piper")
}

/// Downloads whatever is missing, verifying each file before it is moved into
/// place, and returns the installed paths.
pub fn install(root: &Path) -> Result<PiperPaths, String> {
    let engine = ENGINE
        .as_ref()
        .ok_or("no Piper build is pinned for this platform")?;
    let paths = PiperPaths::under(root);
    fs::create_dir_all(root).map_err(|e| format!("could not create {}: {e}", root.display()))?;
    let agent = agent();

    if !paths.executable.is_file() {
        let archive = root.join("engine.download");
        download_verified(&agent, engine, &archive)?;
        let staging = root.join("engine.staging");
        let _ = fs::remove_dir_all(&staging);
        let extracted = extract_archive(&archive, &staging);
        let _ = fs::remove_file(&archive);
        extracted?;
        let engine_dir = root.join("engine");
        let _ = fs::remove_dir_all(&engine_dir);
        fs::rename(&staging, &engine_dir).map_err(|e| format!("could not install engine: {e}"))?;
    }

    let voices = root.join("voices");
    fs::create_dir_all(&voices).map_err(|e| format!("could not create voices dir: {e}"))?;
    for (asset, name) in [
        (&VOICE_CARD, "MODEL_CARD".to_string()),
        (&VOICE_CONFIG, format!("{VOICE_NAME}.onnx.json")),
        // The model last: its presence marks a complete voice.
        (&VOICE_MODEL, format!("{VOICE_NAME}.onnx")),
    ] {
        let dest = voices.join(&name);
        if !dest.is_file() {
            download_verified(&agent, asset, &dest)?;
        }
    }

    if paths.is_installed() {
        Ok(paths)
    } else {
        Err("Piper files are missing after install".into())
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15 * 60)))
        .build()
        .into()
}

/// Streams `asset` to `<dest>.part`, checks its SHA-256, then renames it to
/// `dest`. A mismatch deletes the partial file.
fn download_verified(agent: &ureq::Agent, asset: &Asset, dest: &Path) -> Result<(), String> {
    let response = agent
        .get(asset.url)
        .call()
        .map_err(|e| format!("download of {} failed: {e}", asset.url))?;
    let reader = response.into_body().into_reader();
    write_verified(reader, asset.sha256, dest).map_err(|e| format!("{}: {e}", asset.url))
}

/// Copies `reader` to `<dest>.part` and moves it to `dest` only when its
/// SHA-256 equals `expected`.
pub fn write_verified(mut reader: impl Read, expected: &str, dest: &Path) -> Result<(), String> {
    let mut part = dest.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    let result = (|| {
        let mut file = File::create(&part).map_err(|e| format!("could not write file: {e}"))?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let n = reader
                .read(&mut buffer)
                .map_err(|e| format!("download interrupted: {e}"))?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            file.write_all(&buffer[..n])
                .map_err(|e| format!("could not write file: {e}"))?;
        }
        file.sync_all()
            .map_err(|e| format!("could not write file: {e}"))?;
        let actual = hex(&hasher.finalize());
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(format!(
                "checksum mismatch: expected {expected}, got {actual}"
            ));
        }
        fs::rename(&part, dest).map_err(|e| format!("could not move file into place: {e}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Joins an archive entry name onto `base`, refusing anything that could
/// leave it (absolute paths, drive prefixes, `..`).
pub fn safe_join(base: &Path, name: &str) -> Result<PathBuf, String> {
    if name.starts_with(['/', '\\']) {
        return Err(format!("unsafe path in archive: {name}"));
    }
    let mut out = base.to_path_buf();
    for part in name.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        let component = Path::new(part);
        let mut parts = component.components();
        match (parts.next(), parts.next()) {
            (Some(Component::Normal(_)), None) if !part.contains(':') => out.push(part),
            _ => return Err(format!("unsafe path in archive: {name}")),
        }
    }
    Ok(out)
}

#[cfg(windows)]
fn extract_archive(archive: &Path, dest: &Path) -> Result<(), String> {
    extract_zip(File::open(archive).map_err(|e| e.to_string())?, dest)
}

#[cfg(not(windows))]
fn extract_archive(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| e.to_string())?;
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    for entry in tar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        if !entry.unpack_in(dest).map_err(|e| e.to_string())? {
            return Err("unsafe path in archive".into());
        }
    }
    Ok(())
}

/// Extracts a zip into `dest`, writing regular files only.
pub fn extract_zip(source: impl Read + io::Seek, dest: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(source).map_err(|e| format!("bad archive: {e}"))?;
    fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| format!("bad archive: {e}"))?;
        let target = safe_join(dest, entry.name())?;
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = File::create(&target).map_err(|e| e.to_string())?;
        io::copy(&mut entry, &mut out).map_err(|e| format!("could not extract: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kj-tts-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const HELLO_SHA: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    #[test]
    fn matching_checksum_moves_the_file_into_place() {
        let dir = temp_dir("ok");
        let dest = dir.join("hello.txt");
        write_verified(Cursor::new(b"hello"), HELLO_SHA, &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"hello");
        assert!(!dir.join("hello.txt.part").exists());
    }

    #[test]
    fn wrong_checksum_leaves_nothing_behind() {
        let dir = temp_dir("bad");
        let dest = dir.join("hello.txt");
        let error = write_verified(Cursor::new(b"hellp"), HELLO_SHA, &dest).unwrap_err();
        assert!(error.contains("checksum mismatch"), "{error}");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    }

    #[test]
    fn safe_join_accepts_nested_names_and_rejects_escapes() {
        let base = Path::new("base");
        assert_eq!(
            safe_join(base, "piper/data/x.bin").unwrap(),
            base.join("piper").join("data").join("x.bin")
        );
        for bad in [
            "../evil",
            "a/../../evil",
            "/etc/passwd",
            "C:\\x",
            "a\\..\\..\\x",
            "C:evil",
        ] {
            assert!(safe_join(base, bad).is_err(), "{bad} should be rejected");
        }
    }

    fn zip_with(names: &[&str]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in names {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"data").unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn zip_extracts_nested_files() {
        let dir = temp_dir("zip-ok");
        extract_zip(
            Cursor::new(zip_with(&["piper/piper.exe", "piper/d/a.txt"])),
            &dir,
        )
        .unwrap();
        assert_eq!(fs::read(dir.join("piper/d/a.txt")).unwrap(), b"data");
    }

    #[test]
    fn zip_with_a_traversal_entry_is_rejected_and_writes_nothing_outside() {
        let dir = temp_dir("zip-bad");
        let inner = dir.join("inner");
        let error = extract_zip(Cursor::new(zip_with(&["../escaped.txt"])), &inner).unwrap_err();
        assert!(error.contains("unsafe path"), "{error}");
        assert!(!dir.join("escaped.txt").exists());
    }

    #[test]
    fn installed_layout_needs_engine_and_voice() {
        let dir = temp_dir("layout");
        let paths = PiperPaths::under(&dir);
        assert!(!paths.is_installed());
        for file in [&paths.executable, &paths.model, &paths.config] {
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, b"x").unwrap();
        }
        assert!(paths.is_installed());
    }

    #[test]
    fn pinned_urls_use_the_pinned_voice_revision() {
        for asset in [&VOICE_MODEL, &VOICE_CONFIG, &VOICE_CARD] {
            assert!(asset.url.contains(VOICES_REV));
            assert_eq!(asset.sha256.len(), 64);
        }
    }
}
