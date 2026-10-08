use serde_json::{Value, json};
use std::{
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use yss_plugin_protocol::{PROTOCOL_MAJOR, PROTOCOL_MINOR, PluginFailure, PluginManifest};
use yss_plugin_sdk::{Handler, Peer};

pub struct PluginProcess {
    pub instance_id: String,
    pub peer: Arc<Peer>,
    child: Mutex<Option<Child>>,
    pub leases: AtomicUsize,
    pub(super) diagnostics: Arc<crate::diagnostics::DiagnosticBuffer>,
    #[cfg(windows)]
    job: ProcessJob,
}
impl PluginProcess {
    pub(super) fn spawn(
        manifest: &PluginManifest,
        package: &Path,
        data: &Path,
        instance_id: String,
        handler: Handler,
        diagnostics: Arc<crate::diagnostics::DiagnosticBuffer>,
    ) -> Result<Arc<Self>, PluginFailure> {
        let mut command = Command::new(package.join(&manifest.executable));
        command
            .arg("--data-dir")
            .arg(data)
            .current_dir(package)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command.env_clear();
        for key in [
            "PATH",
            "SystemRoot",
            "WINDIR",
            "USERPROFILE",
            "APPDATA",
            "LOCALAPPDATA",
            "TEMP",
            "TMP",
            "HOMEDRIVE",
            "HOMEPATH",
            "ProgramFiles",
            "ProgramFiles(x86)",
        ] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000 | 0x00000004);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn().map_err(|error| {
            diagnostics
                .push(format!("plugin_start_failed: {:?}\n", error.raw_os_error()).as_bytes());
            PluginFailure::new("plugin_start_failed")
        })?;
        if let Some(mut stderr) = child.stderr.take() {
            let diagnostics = diagnostics.clone();
            std::thread::spawn(move || {
                let mut bytes = [0u8; 4096];
                while let Ok(count) = stderr.read(&mut bytes) {
                    if count == 0 {
                        break;
                    }
                    diagnostics.push(&bytes[..count]);
                }
            });
        }
        #[cfg(windows)]
        let job = match ProcessJob::assign_and_resume(&child) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        let input = child
            .stdin
            .take()
            .ok_or_else(|| PluginFailure::new("plugin_start_failed"))?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| PluginFailure::new("plugin_start_failed"))?;
        let peer = Peer::connect(output, input, manifest.resource_budget.clone(), handler);
        let process = Arc::new(Self {
            instance_id,
            peer,
            child: Mutex::new(Some(child)),
            leases: AtomicUsize::new(0),
            diagnostics,
            #[cfg(windows)]
            job,
        });
        let weak = Arc::downgrade(&process);
        std::thread::spawn(move || {
            loop {
                let Some(process) = weak.upgrade() else {
                    return;
                };
                let exited = process.child.lock().map_or(true, |mut child| {
                    child.as_mut().is_none_or(child_has_exited)
                });
                if exited || !process.peer.is_alive() {
                    process.stop();
                    return;
                }
                drop(process);
                std::thread::park_timeout(Duration::from_millis(100));
            }
        });
        let response = process.request(
            "lifecycle.initialize",
            json!({"input":{"protocolMajor": PROTOCOL_MAJOR, "protocolMinor": PROTOCOL_MINOR,"resourceBudget":manifest.resource_budget}}),
            Duration::from_secs(10),
        ).inspect_err(|error| process.diagnostics.push(format!("lifecycle.initialize: {}\n", error.code).as_bytes()))?;
        if response["pluginId"].as_str() != Some(&manifest.id)
            || response["version"].as_str() != Some(&manifest.version)
            || response["protocolMajor"].as_u64() != Some(u64::from(PROTOCOL_MAJOR))
        {
            process.stop();
            return Err(PluginFailure::new("plugin_protocol_incompatible"));
        }
        Ok(process)
    }
    pub fn is_running(&self) -> bool {
        self.peer.is_alive()
    }
    pub fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, PluginFailure> {
        self.peer.call(method, params, timeout)
    }
    pub fn busy(&self) -> bool {
        self.leases.load(Ordering::Acquire) != 0 || self.peer.pending_count() != 0
    }
    pub fn stop(&self) {
        self.peer.close();
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job.0, 1);
        }
        stop_child(&self.child);
    }
}
impl Drop for PluginProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(not(unix))]
fn child_has_exited(child: &mut Child) -> bool {
    !matches!(child.try_wait(), Ok(None))
}

#[cfg(unix)]
fn child_has_exited(child: &mut Child) -> bool {
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
    loop {
        // Keep the group leader unreaped so its ID cannot be reused before stop.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id() as libc::id_t,
                info.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == 0 {
            return unsafe { info.assume_init() }.si_signo == libc::SIGCHLD;
        }
        if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return true;
        }
    }
}

fn stop_child(slot: &Mutex<Option<Child>>) {
    let child = slot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some(mut child) = child {
        #[cfg(unix)]
        {
            // Spawn gave this unreaped child its own group; consume it only once.
            let result = unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) };
            if result != 0 {
                let _ = child.kill();
            }
        }
        #[cfg(not(unix))]
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(all(test, unix))]
mod unix_process_tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        os::unix::process::CommandExt,
        sync::mpsc,
        time::Instant,
    };

    struct Fixture(Mutex<Option<Child>>);

    impl Drop for Fixture {
        fn drop(&mut self) {
            stop_child(&self.0);
        }
    }

    fn fixture(exit_leader: bool) -> (Fixture, mpsc::Receiver<()>) {
        let tail = if exit_leader { "exit 0" } else { "wait" };
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(format!("sleep 2 & printf 'ready\\n'; {tail}"))
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut ready = String::new();
        reader.read_line(&mut ready).unwrap();
        assert_eq!(ready, "ready\n");
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            std::io::copy(&mut reader, &mut std::io::sink()).unwrap();
            let _ = sender.send(());
        });
        (Fixture(Mutex::new(Some(child))), receiver)
    }

    #[test]
    fn stopping_a_plugin_closes_descendant_pipes_and_retires_the_handle_once() {
        let (fixture, eof) = fixture(false);
        stop_child(&fixture.0);
        assert!(fixture.0.lock().unwrap().is_none());
        stop_child(&fixture.0);
        let stopped = eof.recv_timeout(Duration::from_millis(300)).is_ok();
        if !stopped {
            eof.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        assert!(
            stopped,
            "a plugin descendant retained its output pipe after stop"
        );
    }

    #[test]
    fn exited_leader_remains_waitable_until_its_group_is_stopped() {
        let (fixture, eof) = fixture(true);
        let result = {
            let mut slot = fixture.0.lock().unwrap();
            let child = slot.as_mut().unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !child_has_exited(child) {
                assert!(Instant::now() < deadline, "plugin leader did not exit");
                std::thread::sleep(Duration::from_millis(1));
            }
            let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
            // WNOWAIT verifies ownership without consuming the leader's identity.
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.id() as libc::id_t,
                    info.as_mut_ptr(),
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            }
        };
        stop_child(&fixture.0);
        eof.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(result, 0, "exit polling reaped the process-group leader");
    }
}

#[cfg(windows)]
struct ProcessJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for ProcessJob {}
#[cfg(windows)]
unsafe impl Sync for ProcessJob {}
#[cfg(windows)]
impl ProcessJob {
    fn assign_and_resume(child: &Child) -> Result<Self, PluginFailure> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::{
            Foundation::*,
            System::{Diagnostics::ToolHelp::*, JobObjects::*, Threading::*},
        };
        // The child is suspended until the job owns it, so native plugin code
        // cannot create a child outside the kill-on-close process tree.
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(PluginFailure::new("plugin_start_failed"));
            }
            let job = Self(handle);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags =
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
            info.BasicLimitInformation.ActiveProcessLimit = 8;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&info) as u32,
            ) == 0
                || AssignProcessToJobObject(handle, child.as_raw_handle()) == 0
            {
                return Err(PluginFailure::new("plugin_start_failed"));
            }
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return Err(PluginFailure::new("plugin_start_failed"));
            }
            let mut entry: THREADENTRY32 = std::mem::zeroed();
            entry.dwSize = std::mem::size_of_val(&entry) as u32;
            let mut has_thread = Thread32First(snapshot, &mut entry);
            let mut resumed = false;
            while has_thread != 0 {
                if entry.th32OwnerProcessID == child.id() {
                    let thread = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                    if !thread.is_null() {
                        resumed = ResumeThread(thread) != u32::MAX;
                        CloseHandle(thread);
                    }
                    break;
                }
                has_thread = Thread32Next(snapshot, &mut entry);
            }
            CloseHandle(snapshot);
            if !resumed {
                return Err(PluginFailure::new("plugin_start_failed"));
            }
            Ok(job)
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
