//! Windows Job Object Process Lifecycle Guardian
//!
//! Enforces operating-system-level process tree termination using Windows Job Objects.
//! When `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` is configured, Windows guarantees that
//! when the parent process (or the Job Object handle) is closed, all child and grandchild
//! processes assigned to the job are unconditionally terminated by the OS kernel.
//!
//! Cross-platform: On non-Windows platforms, all functions are safe no-ops.

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;
    use std::sync::OnceLock;
    use tracing::{error, info, warn};

    type HANDLE = *mut c_void;
    type BOOL = i32;
    type DWORD = u32;

    const NULL: HANDLE = std::ptr::null_mut();
    const FALSE: BOOL = 0;

    // Process access rights
    const PROCESS_TERMINATE: DWORD = 0x0001;
    const PROCESS_SET_QUOTA: DWORD = 0x0100;

    // Job Object Limits
    const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: DWORD = 0x0000_2000;
    const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: u32 = 9;

    #[repr(C)]
    #[derive(Default)]
    struct IO_COUNTERS {
        read_operation_count: u64,
        write_operation_count: u64,
        other_operation_count: u64,
        read_transfer_count: u64,
        write_transfer_count: u64,
        other_transfer_count: u64,
    }

    #[repr(C)]
    #[derive(Default)]
    struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
        per_process_user_time_limit: i64,
        per_job_user_time_limit: i64,
        limit_flags: DWORD,
        minimum_working_set_size: usize,
        maximum_working_set_size: usize,
        active_process_limit: DWORD,
        affinity: usize,
        priority_class: DWORD,
        scheduling_class: DWORD,
    }

    #[repr(C)]
    #[derive(Default)]
    struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
        basic_limit_information: JOBOBJECT_BASIC_LIMIT_INFORMATION,
        io_info: IO_COUNTERS,
        process_memory_limit: usize,
        job_memory_limit: usize,
        peak_process_memory_used: usize,
        peak_job_memory_used: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(lpJobAttributes: *const c_void, lpName: *const u16) -> HANDLE;
        fn SetInformationJobObject(
            hJob: HANDLE,
            JobObjectInformationClass: u32,
            lpJobObjectInformation: *const c_void,
            cbJobObjectInformationLength: u32,
        ) -> BOOL;
        fn AssignProcessToJobObject(hJob: HANDLE, hProcess: HANDLE) -> BOOL;
        fn OpenProcess(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwProcessId: DWORD) -> HANDLE;
        fn TerminateProcess(hProcess: HANDLE, uExitCode: u32) -> BOOL;
        fn CloseHandle(hObject: HANDLE) -> BOOL;
        fn GetLastError() -> DWORD;
    }

    /// Thread-safe RAII wrapper for a Windows Job Object handle
    pub struct JobObject {
        handle: HANDLE,
    }

    unsafe impl Send for JobObject {}
    unsafe impl Sync for JobObject {}

    impl JobObject {
        /// Create a new Job Object with `KILL_ON_JOB_CLOSE` configured
        pub fn new() -> Result<Self, String> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle == NULL {
                    let err = GetLastError();
                    return Err(format!("CreateJobObjectW failed with error code {}", err));
                }

                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

                let ok = SetInformationJobObject(
                    handle,
                    JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                    &info as *const _ as *const c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                );

                if ok == FALSE {
                    let err = GetLastError();
                    CloseHandle(handle);
                    return Err(format!(
                        "SetInformationJobObject (KILL_ON_JOB_CLOSE) failed with error code {}",
                        err
                    ));
                }

                info!("Windows Job Object initialized with KILL_ON_JOB_CLOSE");
                Ok(JobObject { handle })
            }
        }

        /// Assign a process handle to this Job Object
        pub fn assign_process(&self, process_handle: HANDLE) -> bool {
            unsafe {
                let ok = AssignProcessToJobObject(self.handle, process_handle);
                ok != FALSE
            }
        }

        /// Assign a child process by PID to this Job Object
        pub fn assign_pid(&self, pid: u32) -> bool {
            unsafe {
                let h_proc = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, FALSE, pid);
                if h_proc == NULL {
                    let err = GetLastError();
                    warn!("OpenProcess failed for PID {}: error code {}", pid, err);
                    return false;
                }
                let ok = self.assign_process(h_proc);
                if !ok {
                    let err = GetLastError();
                    warn!("AssignProcessToJobObject failed for PID {}: error code {}", pid, err);
                } else {
                    info!("Assigned PID {} to Windows Job Object guardian", pid);
                }
                CloseHandle(h_proc);
                ok
            }
        }
    }

    impl Drop for JobObject {
        fn drop(&mut self) {
            unsafe {
                if self.handle != NULL {
                    CloseHandle(self.handle);
                    info!("Windows Job Object closed; OS kernel will terminate bound child processes");
                }
            }
        }
    }

    static GLOBAL_JOB: OnceLock<JobObject> = OnceLock::new();

    /// Get or initialize the global guardian Job Object
    pub fn get_global_job() -> Option<&'static JobObject> {
        GLOBAL_JOB.get_or_init(|| {
            match JobObject::new() {
                Ok(job) => job,
                Err(e) => {
                    error!("Failed to initialize global Windows Job Object: {}", e);
                    panic!("Job Object initialization failed: {}", e);
                }
            }
        });
        GLOBAL_JOB.get()
    }

    /// Assign a PID to the global Job Object so it is killed when buzz-agent terminates
    pub fn assign_pid_to_job(pid: u32) -> bool {
        if let Some(job) = get_global_job() {
            job.assign_pid(pid)
        } else {
            false
        }
    }

    /// Forcefully terminate a process by PID immediately on Windows
    pub fn terminate_pid(pid: u32) -> bool {
        unsafe {
            let h_proc = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
            if h_proc == NULL {
                return false;
            }
            let ok = TerminateProcess(h_proc, 1);
            CloseHandle(h_proc);
            ok != FALSE
        }
    }
}

// Cross-platform public API exports
#[cfg(windows)]
pub use imp::{assign_pid_to_job, get_global_job, terminate_pid, JobObject};

#[cfg(not(windows))]
pub fn assign_pid_to_job(_pid: u32) -> bool {
    true // No-op on Unix
}

#[cfg(not(windows))]
pub fn terminate_pid(_pid: u32) -> bool {
    true // No-op on Unix
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn test_job_object_creation_and_global_singleton() {
        let job = JobObject::new();
        assert!(job.is_ok(), "JobObject::new should succeed on Windows");

        let global = get_global_job();
        assert!(global.is_some(), "get_global_job should return valid singleton");
    }

    #[test]
    #[cfg(windows)]
    fn test_assign_current_process_to_job() {
        let current_pid = std::process::id();
        let ok = assign_pid_to_job(current_pid);
        // Current process may or may not allow assignment depending on whether it's already in an incompatible job,
        // but it must not panic or crash.
        let _ = ok;
    }
}
