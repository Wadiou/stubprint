//! `stubprint update`: runs the installer of a release into the folder this program is in, which
//! replaces it. The installer checks what it downloads against the checksums of the release.
//!
//! Nothing here is compiled for one system only, so a build on any of them checks the code of all.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Args;

use super::{say, Failure};

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Install this version, such as `0.4.0`, and not the latest one.
    #[arg(long, value_name = "VERSION", value_parser = version_number)]
    to: Option<String>,
}

/// A version as the releases are called, such as `0.4.0` or `1.2.3-rc.1`, with or without the `v`.
fn version_number(text: &str) -> Result<String, String> {
    let number = text.strip_prefix('v').unwrap_or(text);
    let well_formed = number.starts_with(|c: char| c.is_ascii_digit())
        && number
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    match well_formed {
        true => Ok(number.to_string()),
        false => Err(format!("`{text}` is not a version like 0.4.0")),
    }
}

/// The installer that a release attaches for this system, of the latest release when no version is
/// given.
fn installer_url(version: Option<&str>) -> String {
    let file = match cfg!(windows) {
        true => "stubprint-installer.ps1",
        false => "stubprint-installer.sh",
    };
    match version {
        Some(version) => format!("{REPOSITORY}/releases/download/v{version}/{file}"),
        None => format!("{REPOSITORY}/releases/latest/download/{file}"),
    }
}

pub fn update(args: &UpdateArgs) -> Result<(), Failure> {
    let program = super::installed_program()?;
    let folder = program
        .parent()
        .ok_or_else(|| format!("{} is not in a folder", program.display()))?;

    let url = installer_url(args.to.as_deref());
    say(format!("Updating {} from {url}", program.display()));

    let extension = if cfg!(windows) { "ps1" } else { "sh" };
    let installer = std::env::temp_dir().join(format!(
        "stubprint-installer-{}.{extension}",
        std::process::id()
    ));
    let result = download(&url, &installer).and_then(|()| install(&installer, folder, &program));
    let _ = std::fs::remove_file(&installer);
    result?;

    let before = env!("CARGO_PKG_VERSION");
    let after = installed_version(&program)?;
    if after == before {
        say(format!("stubprint is already at {after}"));
    } else {
        say(format!("stubprint is now at {after}, it was {before}"));
        say(format!(
            "A server that is running keeps {before} until it is started again."
        ));
    }
    Ok(())
}

/// Saves the installer to a file, with curl or wget on most systems and PowerShell on Windows.
fn download(url: &str, to: &Path) -> Result<(), Failure> {
    if cfg!(windows) {
        let script = format!(
            "$ProgressPreference = 'SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri '{url}' -OutFile '{}'",
            to.display().to_string().replace('\'', "''")
        );
        let status = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .status()
            .map_err(|error| format!("cannot run powershell: {error}"))?;
        return match status.success() {
            true => Ok(()),
            false => Err(format!("cannot download {url}")),
        };
    }
    for (tool, flags) in [("curl", "-fsSL -o"), ("wget", "-qO")] {
        match Command::new(tool)
            .args(flags.split(' '))
            .arg(to)
            .arg(url)
            .status()
        {
            Ok(status) if status.success() => return Ok(()),
            Ok(_) => return Err(format!("cannot download {url}")),
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("cannot run {tool}: {error}")),
        }
    }
    Err("curl or wget is needed to download the update".into())
}

/// Runs the installer so that it puts the new program where this one is.
fn install(installer: &Path, folder: &Path, program: &Path) -> Result<(), Failure> {
    if !cfg!(windows) {
        let status = Command::new("sh")
            .arg(installer)
            .arg("--dir")
            .arg(folder)
            .status()
            .map_err(|error| format!("cannot run the installer: {error}"))?;
        return match status.success() {
            true => Ok(()),
            false => Err("the installer failed, and the program was not changed".into()),
        };
    }

    // Windows lets a running program be renamed but not written over, so it steps aside first.
    let aside: PathBuf = program.with_extension("exe.old");
    let _ = std::fs::remove_file(&aside);
    std::fs::rename(program, &aside)
        .map_err(|error| format!("cannot move {} aside: {error}", program.display()))?;
    let status = Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(installer)
        .env("STUBPRINT_INSTALL_DIR", folder)
        .status();
    match status {
        Ok(status) if status.success() && program.exists() => Ok(()),
        other => {
            let _ = std::fs::remove_file(program);
            let _ = std::fs::rename(&aside, program);
            match other {
                Err(error) => Err(format!("cannot run the installer: {error}")),
                Ok(_) => Err("the installer failed, and the program was not changed".into()),
            }
        }
    }
}

/// The version of the program that is now at `path`, which is the new one after an update.
fn installed_version(path: &Path) -> Result<String, Failure> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .map_err(|error| format!("cannot run the updated program: {error}"))?;
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .last()
        .map(str::to_string)
        .ok_or_else(|| "the updated program did not say its version".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_is_a_number_with_or_without_the_v() {
        assert_eq!(version_number("0.4.0").unwrap(), "0.4.0");
        assert_eq!(version_number("v1.2.3-rc.1").unwrap(), "1.2.3-rc.1");
        for bad in ["latest", "1.0;rm", "../0.4.0", "", "v", "0 4"] {
            assert!(
                version_number(bad).is_err(),
                "{bad} should not be a version"
            );
        }
    }

    #[test]
    fn the_installer_is_the_one_of_the_latest_release_or_of_the_version() {
        let file = if cfg!(windows) {
            "stubprint-installer.ps1"
        } else {
            "stubprint-installer.sh"
        };
        assert_eq!(
            installer_url(None),
            format!("{REPOSITORY}/releases/latest/download/{file}")
        );
        assert_eq!(
            installer_url(Some("0.4.0")),
            format!("{REPOSITORY}/releases/download/v0.4.0/{file}")
        );
    }
}
