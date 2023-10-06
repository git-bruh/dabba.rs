use nix::mount::MntFlags;
use nix::mount::MsFlags;
use nix::sched::CloneFlags;
use nix::sys::signal::Signal;
use std::ffi::CString;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn in_sandbox() -> isize {
    let path = std::env::args().nth(1).expect("no path!");

    nix::mount::mount(
        Some(path.as_str()),
        path.as_str(),
        None::<&str>,
        MsFlags::MS_REC | MsFlags::MS_BIND,
        None::<&str>,
    )
    .expect("failed to mount!");

    nix::unistd::chdir(path.as_str()).expect("failed to chdir!");
    nix::mount::mount(
        Some("proc"),
        "proc",
        Some("proc"),
        MsFlags::empty(),
        None::<&str>,
    )
    .expect("failed to mount /proc!");
    nix::unistd::pivot_root(".", ".").expect("failed to pivot!");
    nix::mount::umount2(".", MntFlags::MNT_DETACH).expect("failed to unmount old root!");

    let argv: [CString; 1] = [CString::new("sh").unwrap()];
    let env: [CString; 0] = [];

    nix::unistd::execve(&CString::new("/bin/sh").unwrap(), &argv, &env).expect("failed to exec!");

    0
}

fn make_root(pid: i32) -> Result<(), std::io::Error> {
    // /proc/<pid>
    let mut path = PathBuf::new();

    path.push("/proc");
    path.push(pid.to_string());

    // We must never allow setgroups() to be called as that could potentially
    // allow access to otherwise restricted resources
    let mut setgroups = File::create(path.join("setgroups"))?;
    setgroups.write_all(b"deny")?;

    println!("Mapping UID 1000 to 0 (root) in sandbox");

    // /proc/<pid>/uid_map
    let mut uid_map = File::create(path.join("uid_map"))?;
    uid_map.write_all(b"0 1000 1")?;

    println!("Mapping GID 1000 to 0 (root) in sandbox");

    // /proc/<pid>/gid_map
    let mut gid_map = File::create(path.join("gid_map"))?;
    gid_map.write_all(b"0 1000 1")?;

    Ok(())
}

fn main() {
    let mut stack = [0_u8; 1024 * 1024];

    let pid = unsafe {
        nix::sched::clone(
            Box::new(in_sandbox),
            &mut stack,
            // Mount namespace
            CloneFlags::CLONE_NEWNS
                // PID namespace
                | CloneFlags::CLONE_NEWPID
                // New network namespace
                | CloneFlags::CLONE_NEWNET
                // New user namespace, allowing us to run this rootlessly
                | CloneFlags::CLONE_NEWUSER,
            // Ensure same semantics as fork()
            Some(Signal::SIGCHLD as i32),
        )
        .expect("failed to clone!")
    };

    println!("Spawned sandbox with PID: {pid}");

    // make_root(pid.as_raw()).expect("failed to make uid 0!");

    nix::sys::wait::waitpid(pid, None).expect("failed to wait!");
}
