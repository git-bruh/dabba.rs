use nix::mount::MsFlags;
use nix::sched::CloneFlags;
use nix::sys::signal::Signal;
use std::ffi::CString;

fn in_sandbox() -> isize {
    let argv: [CString; 1] = [CString::new("sh").unwrap()];
    let env: [CString; 0] = [];

    nix::mount::mount(
        Some("proc"),
        "/proc",
        Some("proc"),
        MsFlags::empty(),
        None::<&str>,
    )
    .expect("failed to mount /proc!");

    nix::unistd::execve(&CString::new("/bin/sh").unwrap(), &argv, &env).expect("failed to exec!");

    0
}

fn main() {
    static mut STACK: [u8; 1024 * 1024] = [0_u8; 1024 * 1024];

    let pid = unsafe {
        nix::sched::clone(
            Box::new(in_sandbox),
            &mut STACK,
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

    nix::sys::wait::waitpid(pid, None).expect("failed to wait!");
}
