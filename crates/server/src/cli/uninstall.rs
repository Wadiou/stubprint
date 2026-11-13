//! `stubprint uninstall`: removes the installed program, and with `--purge` the database it kept.
//!
//! Nothing here is compiled for one system only, so a build on any of them checks the code of all.

use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Args;

use super::{say, Failure};

#[derive(Args, Debug)]
pub struct UninstallArgs {
    /// Also delete the database: the printers, the receipts and the connections it kept. Without
    /// it, the database is left, so installing again finds them.
    #[arg(long)]
    purge: bool,
}

pub fn uninstall(args: &UninstallArgs) -> Result<(), Failure> {
    let program = super::installed_program()?;
    say(format!("Uninstalling {}", program.display()));

    if args.purge {
        remove_data()?;
    }
    remove(&program)?;

    say("stubprint is uninstalled.");
    if !args.purge {
        if let Some(folder) = data_folder() {
            say(format!(
                "{} is kept: remove it by hand, or run `uninstall --purge` next time.",
                folder.display()
            ));
        }
    }
    Ok(())
}

/// The folder the database is kept in, when this system has one.
fn data_folder() -> Option<PathBuf> {
    stubprint_server::default_database()?
        .parent()
        .map(Path::to_path_buf)
}

fn remove_data() -> Result<(), Failure> {
    let Some(folder) = data_folder() else {
        return Ok(());
    };
    match std::fs::remove_dir_all(&folder) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot remove {}: {error}", folder.display())),
    }
}

/// Deletes the program's own file.
fn remove(program: &Path) -> Result<(), Failure> {
    // A file can be deleted while it is running: the name goes, and the running copy, which
    // already has the file open, keeps going until it exits by itself.
    if !cfg!(windows) {
        return std::fs::remove_file(program)
            .map_err(|error| format!("cannot remove {}: {error}", program.display()));
    }

    // Windows refuses to delete a running .exe, but it does allow renaming one. Rename it out of
    // the way first, so the program is gone from its real name at once.
    let aside: PathBuf = program.with_extension("exe.old");
    let _ = std::fs::remove_file(&aside); // a leftover from a previous attempt, if any
    std::fs::rename(program, &aside)
        .map_err(|error| format!("cannot rename {}: {error}", program.display()))?;

    // The renamed file is still open by this process, so deleting it needs a process that outlives
    // this one. `spawn` starts `cmd` and returns at once, without waiting for it to finish, so this
    // process exits right after while `cmd` is still only pinging. By the time `cmd` reaches
    // `del`, this process is gone and nothing has the file open any more.
    let folder = program.parent().map(Path::to_path_buf);
    if let Some(folder) = &folder {
        leave_path(folder);
    }
    // `rmdir` removes the folder only when nothing else is in it.
    let rmdir = folder
        .map(|folder| format!(" & rmdir \"{}\"", folder.display()))
        .unwrap_or_default();
    let _ = Command::new("cmd")
        .args([
            "/C",
            // `ping -n 2` to localhost is a crude sleep of about a second: Windows has no plain
            // sleep command. `&` runs `del` only after it, in the same `cmd` line.
            &format!(
                "ping -n 2 127.0.0.1 >NUL & del /F /Q \"{}\"{rmdir}",
                aside.display()
            ),
        ])
        .spawn();
    Ok(())
}

/// Takes the folder out of the PATH of this user, where the installer put it.
fn leave_path(folder: &Path) {
    let script = format!(
        "$f = '{}'; $p = [Environment]::GetEnvironmentVariable('Path', 'User'); \
         $k = ($p -split ';') | Where-Object {{ $_ -and $_ -ne $f }}; \
         [Environment]::SetEnvironmentVariable('Path', ($k -join ';'), 'User')",
        folder.display().to_string().replace('\'', "''")
    );
    let _ = Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .status();
}
