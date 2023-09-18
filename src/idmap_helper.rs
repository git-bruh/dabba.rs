use nix::unistd::{Gid, Pid, Uid};
use std::process::Command;

/// Mappings for the commands
#[derive(Debug)]
struct Mappings {
    pub id: u32,
    pub lowerid: u32,
    pub count: u32,
}

/// The command to execute
enum MapCmd {
    UidMap,
    GidMap,
}

/// Wraps the construction of a cmdline and status code checking
fn exec_cmd_with_mappings(
    command: MapCmd,
    pid: Pid,
    mappings: &[Mappings],
) -> Result<(), std::io::Error> {
    let cmd = match command {
        MapCmd::UidMap => "newuidmap",
        MapCmd::GidMap => "newgidmap",
    };

    // The programs take arguments in groups of 3 after the PID
    // > newuidmap pid uid loweruid count
    // >           [uid loweruid count [ ... ]]
    let mut args = vec![pid.as_raw().to_string()];

    // Find out if there's a more rusty way to do this transformation
    // without involving one allocation per field...
    for mapping in mappings {
        for field in [mapping.id, mapping.lowerid, mapping.count] {
            args.push(field.to_string());
        }
    }

    log::info!("Executing {cmd} with cmdline {args:?}");

    let status = Command::new(cmd).args(args).status()?;

    if !status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("{cmd} exited with {status}"),
        ));
    }

    Ok(())
}

/// Setup UID GID mappings using the external `new{uid,gid}map` SUID programs
pub fn setup_maps(sandbox_pid: Pid) -> Result<(), std::io::Error> {
    // TODO make this configurable
    let uid_mappings = [
        Mappings {
            id: 0,
            lowerid: Uid::current().as_raw(),
            count: 1,
        },
        Mappings {
            id: 1,
            lowerid: 100000,
            count: 65536,
        },
    ];

    exec_cmd_with_mappings(MapCmd::UidMap, sandbox_pid, &uid_mappings)?;

    let gid_mappings = [
        Mappings {
            id: 0,
            lowerid: Gid::current().as_raw(),
            count: 1,
        },
        Mappings {
            id: 1,
            lowerid: 100000,
            count: 65536,
        },
    ];

    exec_cmd_with_mappings(MapCmd::GidMap, sandbox_pid, &gid_mappings)?;

    Ok(())
}
