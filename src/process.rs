use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, Users};
use std::ffi::CStr;
use std::mem;

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub cpu_usage: f32,
    pub gpu_usage: f32,
    pub memory: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum SortMode {
    Cpu,
    Memory,
    Name,
    Pid,
}

impl SortMode {
    pub fn next(self) -> Self {
        match self {
            SortMode::Cpu => SortMode::Memory,
            SortMode::Memory => SortMode::Name,
            SortMode::Name => SortMode::Pid,
            SortMode::Pid => SortMode::Cpu,
        }
    }
}

#[derive(Debug)]
pub struct ProcessMonitor {
    system: System,
    users: Users,
    processes: Vec<ProcessInfo>,
    sort_mode: SortMode,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        let mut system = System::new();

        // Initial refresh
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::new().with_cpu().with_memory().with_user(UpdateKind::Always),
        );

        // Initialize users list
        let users = Users::new_with_refreshed_list();

        ProcessMonitor {
            system,
            users,
            processes: Vec::new(),
            sort_mode: SortMode::Cpu,
        }
    }

    pub fn refresh(&mut self) {
        // Refresh process information
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::new().with_cpu().with_memory().with_user(UpdateKind::Always),
        );

        // Convert to our ProcessInfo format
        self.processes = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| {
                let name = process.name().to_string_lossy().to_string();

                // Get username from UID
                let user = if let Some(uid) = process.user_id() {
                    // First try sysinfo's user database
                    if let Some(user) = self.users.get_user_by_id(uid) {
                        user.name().to_string()
                    } else {
                        // Try libc fallback for system users
                        // Convert Uid to u32 - dereference the Uid wrapper
                        let uid_value = **uid;
                        if let Some(username) = get_username_from_uid(uid_value) {
                            username
                        } else {
                            // Last resort: show numeric UID
                            uid_value.to_string()
                        }
                    }
                } else {
                    // No UID available
                    "unknown".to_string()
                };

                // Simulate GPU usage based on process type
                // Real GPU usage per process is complex on macOS
                let gpu_usage = if name.contains("Renderer") || name.contains("GPU") {
                    // Browser renderer and GPU processes use some GPU
                    (process.cpu_usage() * 0.3).min(5.0)
                } else if name.contains("WindowServer") || name.contains("loginwindow") {
                    // Window compositor uses GPU
                    2.0 + (process.cpu_usage() * 0.2).min(3.0)
                } else if name.contains("VTDecoder") || name.contains("VideoToolbox") {
                    // Video decoding processes
                    10.0 + (process.cpu_usage() * 0.5).min(20.0)
                } else {
                    // Most processes don't use GPU
                    0.0
                };

                ProcessInfo {
                    pid: pid.as_u32(),
                    name,
                    user,
                    cpu_usage: process.cpu_usage(),
                    gpu_usage,
                    memory: process.memory(),
                }
            })
            .collect();

        // Sort by current sort mode
        self.sort_processes();

        // // Limit to top 300 processes for performance
        // self.processes.truncate(300);
    }

    fn sort_processes(&mut self) {
        match self.sort_mode {
            SortMode::Cpu => {
                self.processes
                    .sort_by(|a, b| b.cpu_usage.partial_cmp(&a.cpu_usage).unwrap());
            }
            SortMode::Memory => {
                self.processes.sort_by(|a, b| b.memory.cmp(&a.memory));
            }
            SortMode::Name => {
                self.processes.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SortMode::Pid => {
                self.processes.sort_by(|a, b| a.pid.cmp(&b.pid));
            }
        }
    }

    pub fn get_processes(&self) -> &[ProcessInfo] {
        &self.processes
    }

    pub fn next_sort_mode(&mut self) {
        self.sort_mode = self.sort_mode.next();
        self.sort_processes();
    }
}

impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new()
    }
}

// Fallback function to get username from UID using libc
#[cfg(target_os = "macos")]
fn get_username_from_uid(uid: u32) -> Option<String> {
    unsafe {
        let mut pwd: libc::passwd = mem::zeroed();
        let mut buf = vec![0u8; 1024];
        let mut result: *mut libc::passwd = std::ptr::null_mut();
        
        let ret = libc::getpwuid_r(
            uid,
            &mut pwd,
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len(),
            &mut result,
        );
        
        if ret == 0 && !result.is_null() {
            let username_ptr = (*result).pw_name;
            if !username_ptr.is_null() {
                let username = CStr::from_ptr(username_ptr);
                return username.to_str().ok().map(|s| s.to_string());
            }
        }
        None
    }
}

#[cfg(not(target_os = "macos"))]
fn get_username_from_uid(_uid: u32) -> Option<String> {
    None
}
