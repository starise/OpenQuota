use std::{
    ffi::OsStr,
    io::{self, Read},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use process_wrap::std::CommandWrap;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Creates a child process intended for background discovery work.
///
/// A GUI-subsystem application does not own a console on Windows. Starting a
/// console-subsystem executable from it without `CREATE_NO_WINDOW` briefly
/// creates a visible console window, so every background child process must go
/// through this helper.
pub fn background_command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    hide_console_window(&mut command);
    command
}

/// Captures a background command with a deadline for the process and its output.
pub fn output_with_timeout(mut command: Command, timeout: Duration) -> io::Result<Output> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "child timeout is too large"))?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut command = CommandWrap::from(command);
    #[cfg(windows)]
    command
        .wrap(process_wrap::std::CreationFlags(CREATE_NO_WINDOW))
        .wrap(process_wrap::std::JobObject);
    #[cfg(unix)]
    command.wrap(process_wrap::std::ProcessGroup::leader());
    let mut child = command.spawn()?;
    let stdout = child
        .stdout()
        .take()
        .ok_or_else(|| io::Error::other("child stdout was not captured"))?;
    let stderr = child
        .stderr()
        .take()
        .ok_or_else(|| io::Error::other("child stderr was not captured"))?;
    let stdout_reader = thread::spawn(move || read_all(stdout));
    let stderr_reader = thread::spawn(move || read_all(stderr));
    let status = loop {
        match child.try_wait() {
            Ok(Some(status))
                if stdout_reader.is_finished()
                    && stderr_reader.is_finished()
                    && Instant::now() < deadline =>
            {
                break status;
            }
            Ok(_) if Instant::now() < deadline => {
                thread::sleep(
                    deadline
                        .saturating_duration_since(Instant::now())
                        .min(Duration::from_millis(10)),
                );
            }
            result => {
                let _ = child.start_kill();
                #[cfg(unix)]
                thread::spawn(move || {
                    let _ = child.wait();
                });
                let error = result.err().unwrap_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "background command timed out")
                });
                return Err(error);
            }
        }
    };

    Ok(Output {
        status,
        stdout: join_reader(stdout_reader)?,
        stderr: join_reader(stderr_reader)?,
    })
}

fn read_all(mut reader: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn join_reader(reader: thread::JoinHandle<io::Result<Vec<u8>>>) -> io::Result<Vec<u8>> {
    reader
        .join()
        .map_err(|_| io::Error::other("background output reader panicked"))?
}

#[cfg(target_os = "windows")]
fn hide_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_console_window(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };

    use super::{background_command, output_with_timeout};

    #[cfg(windows)]
    #[test]
    fn background_console_process_has_no_console_window() {
        const PROBE: &str = "OPENQUOTA_TEST_CONSOLE_PROBE";
        if env::var_os(PROBE).is_some() {
            // GetConsoleWindow takes no arguments and returns null without an attached console.
            let window = unsafe { windows_sys::Win32::System::Console::GetConsoleWindow() };
            assert!(window.is_null(), "background process has a console window");
            println!("console probe passed");
            return;
        }

        let mut command = background_command(env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "child_process::tests::background_console_process_has_no_console_window",
                "--nocapture",
            ])
            .env(PROBE, "1");
        let output = output_with_timeout(command, Duration::from_secs(10)).unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("console probe passed"),
            "{stdout}{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn background_command_deadline_terminates_a_slow_process() {
        const READY: &str = "OPENQUOTA_TEST_SLOW_PROCESS_READY";
        if let Some(path) = env::var_os(READY) {
            fs::write(path, []).unwrap();
            thread::sleep(Duration::from_secs(6));
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let ready = dir.path().join("ready");
        let mut command = background_command(env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "child_process::tests::background_command_deadline_terminates_a_slow_process",
            ])
            .env(READY, &ready);

        let started = Instant::now();
        let error = output_with_timeout(command, Duration::from_secs(2)).unwrap_err();

        assert!(
            ready.is_file(),
            "the child did not start before the deadline"
        );
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(4));
    }

    #[test]
    fn deadline_includes_output_held_open_by_a_descendant() {
        const ROLE: &str = "OPENQUOTA_TEST_PROCESS_ROLE";
        const READY: &str = "OPENQUOTA_TEST_PROCESS_READY";
        let mut command = background_command(env::current_exe().unwrap());
        command.args([
            "--exact",
            "child_process::tests::deadline_includes_output_held_open_by_a_descendant",
            "--nocapture",
        ]);
        match env::var(ROLE).as_deref() {
            Ok("leader") => {
                #[allow(
                    clippy::zombie_processes,
                    reason = "The fixture must exit before its descendant"
                )]
                let _descendant = command.env(ROLE, "descendant").spawn().unwrap();
                return;
            }
            Ok("descendant") => {
                let listener = TcpListener::bind("127.0.0.1:0").unwrap();
                fs::write(
                    env::var_os(READY).unwrap(),
                    listener.local_addr().unwrap().to_string(),
                )
                .unwrap();
                thread::sleep(Duration::from_secs(6));
                return;
            }
            _ => {}
        }
        let dir = tempfile::tempdir().unwrap();
        let ready = dir.path().join("ready");
        command.env(ROLE, "leader").env(READY, &ready);

        let started = Instant::now();
        let error = output_with_timeout(command, Duration::from_secs(2)).unwrap_err();

        assert!(
            ready.is_file(),
            "the descendant did not start before the deadline"
        );
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(4));
        let address = fs::read_to_string(ready).unwrap();
        let cleanup_deadline = Instant::now() + Duration::from_secs(1);
        while TcpListener::bind(&address).is_err() {
            assert!(
                Instant::now() < cleanup_deadline,
                "descendant still holds its listener"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
}
