use std::collections::HashMap;
use std::ffi::CStr;
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::mem;
use std::process::Command;
use std::time::Instant;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, Users};

/// Log timing data to /tmp/oversee-profile.log for performance analysis
fn log_timing(label: &str, duration_ms: u128) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/oversee-profile.log")
    {
        let _ = writeln!(file, "{}: {}ms", label, duration_ms);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy)]
pub enum ConnectionState {
    Listen,
    Established,
    Other,
}

#[derive(Debug, Clone)]
pub struct PortInfo {
    pub port: u16,
    #[allow(dead_code)] // May be used for detailed network info in future
    pub protocol: Protocol,
    pub state: ConnectionState,
    #[allow(dead_code)] // May be used for detailed network info in future
    pub local_address: Option<String>,
    #[allow(dead_code)] // May be used for detailed network info in future
    pub remote_address: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cmd: String,
    pub user: String,
    pub cpu_usage: f32,
    pub gpu_usage: f32,
    pub memory: u64,
    pub ports: Vec<PortInfo>,
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

impl Protocol {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "TCP" => Some(Protocol::Tcp),
            "UDP" => Some(Protocol::Udp),
            _ => None,
        }
    }
}

impl ConnectionState {
    fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "LISTEN" => ConnectionState::Listen,
            "ESTABLISHED" => ConnectionState::Established,
            _ => ConnectionState::Other,
        }
    }
}

fn get_process_ports() -> HashMap<u32, Vec<PortInfo>> {
    let mut port_map = HashMap::new();

    // Run lsof command to get network connections
    let lsof_start = Instant::now();
    let output = match Command::new("lsof").args(["-i", "-P", "-n"]).output() {
        Ok(output) => output,
        Err(_) => return port_map, // lsof not available or failed
    };
    log_timing("lsof_command", lsof_start.elapsed().as_millis());

    if !output.status.success() {
        return port_map;
    }

    let parse_start = Instant::now();
    let output_str = String::from_utf8_lossy(&output.stdout);

    for line in output_str.lines() {
        // Skip header line
        if line.starts_with("COMMAND") {
            continue;
        }

        // Parse standard lsof output line
        if let Some(port_info) = parse_lsof_line(line) {
            port_map
                .entry(port_info.0)
                .or_insert_with(Vec::new)
                .push(port_info.1);
        }
    }
    log_timing("lsof_parse", parse_start.elapsed().as_millis());

    port_map
}

fn parse_lsof_line(line: &str) -> Option<(u32, PortInfo)> {
    // Parse lines like:
    // rapportd   1000 adam    8u  IPv4 0xe349afbd3b2ee8ee      0t0  TCP *:60744 (LISTEN)
    // identitys  1016 adam   18u  IPv4 0x34f005a6e91ac63b      0t0  UDP *:*

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 9 {
        return None;
    }

    // Extract PID (second column, index 1)
    let pid = parts[1].parse::<u32>().ok()?;

    // Extract protocol (8th column, index 7: TCP or UDP)
    let protocol = Protocol::from_str(parts[7])?;

    // Extract address info (9th column, index 8)
    let addr_part = parts[8];

    // Skip non-port entries like "*:*"
    if addr_part == "*:*" {
        return None;
    }

    // Extract state if present (in parentheses at the end)
    let state = if parts.len() > 9 && parts[9].starts_with('(') && parts[9].ends_with(')') {
        ConnectionState::from_str(&parts[9][1..parts[9].len() - 1])
    } else {
        ConnectionState::Other
    };

    // Parse the address part
    let (local_addr, remote_addr) = if let Some(arrow_pos) = addr_part.find("->") {
        // Connection: local->remote
        let local = &addr_part[..arrow_pos];
        let remote = &addr_part[arrow_pos + 2..];
        (Some(local.to_string()), Some(remote.to_string()))
    } else {
        // Listening or single address
        (Some(addr_part.to_string()), None)
    };

    // Extract port from local address
    let port = extract_port(addr_part)?;

    Some((
        pid,
        PortInfo {
            port,
            protocol,
            state,
            local_address: local_addr,
            remote_address: remote_addr,
        },
    ))
}

fn extract_port(addr: &str) -> Option<u16> {
    // Extract port from addresses like:
    // 127.0.0.1:8080
    // *:22
    // [::1]:8080

    if let Some(colon_pos) = addr.rfind(':') {
        let port_str = &addr[colon_pos + 1..];
        port_str.parse().ok()
    } else {
        None
    }
}

#[derive(Debug)]
pub struct ProcessMonitor {
    system: System,
    users: Users,
    processes: Vec<ProcessInfo>,
    sort_mode: SortMode,
    /// Cache UID -> username mappings to avoid repeated FFI calls
    uid_cache: HashMap<u32, String>,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        let mut system = System::new();

        // Initial refresh
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::new()
                .with_cpu()
                .with_memory()
                .with_user(UpdateKind::Always)
                .with_cmd(UpdateKind::OnlyIfNotSet),
        );

        // Initialize users list
        let users = Users::new_with_refreshed_list();

        ProcessMonitor {
            system,
            users,
            processes: Vec::new(),
            sort_mode: SortMode::Cpu,
            uid_cache: HashMap::new(),
        }
    }

    pub fn refresh(&mut self, include_ports: bool) {
        // Refresh process information
        let sysinfo_start = Instant::now();
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::new()
                .with_cpu()
                .with_memory()
                .with_user(UpdateKind::Always)
                .with_cmd(UpdateKind::OnlyIfNotSet),
        );
        log_timing("sysinfo_refresh", sysinfo_start.elapsed().as_millis());

        // Get port information for all processes (expensive operation - only when requested)
        let port_map = if include_ports {
            get_process_ports()
        } else {
            HashMap::new()
        };

        // Convert to our ProcessInfo format
        self.processes = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| {
                let name = process.name().to_string_lossy().to_string();

                // Get full command line, fall back to name if empty
                let cmd_parts: Vec<String> = process
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy().to_string())
                    .collect();
                let cmd = if cmd_parts.is_empty() {
                    name.clone()
                } else {
                    cmd_parts.join(" ")
                };

                // Get username from UID (with caching to avoid repeated FFI calls)
                let user = if let Some(uid) = process.user_id() {
                    let uid_value = **uid;
                    // Check cache first
                    if let Some(cached) = self.uid_cache.get(&uid_value) {
                        cached.clone()
                    } else {
                        // First try sysinfo's user database
                        let username = if let Some(user) = self.users.get_user_by_id(uid) {
                            user.name().to_string()
                        } else if let Some(username) = get_username_from_uid(uid_value) {
                            // Try libc fallback for system users
                            username
                        } else {
                            // Last resort: show numeric UID
                            uid_value.to_string()
                        };
                        // Cache the result
                        self.uid_cache.insert(uid_value, username.clone());
                        username
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

                let process_pid = pid.as_u32();
                let ports = port_map.get(&process_pid).cloned().unwrap_or_default();

                ProcessInfo {
                    pid: process_pid,
                    name,
                    cmd,
                    user,
                    cpu_usage: process.cpu_usage(),
                    gpu_usage,
                    memory: process.memory(),
                    ports,
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
