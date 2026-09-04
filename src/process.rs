use crate::category::{Category, classify};
use std::collections::HashMap;
use std::ffi::CStr;
#[cfg(feature = "profile")]
use std::fs::OpenOptions;
#[cfg(feature = "profile")]
use std::io::Write as IoWrite;
use std::mem;
use std::process::Command;
#[cfg(feature = "profile")]
use std::time::Instant;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind, Users};

/// Log timing data to /tmp/oversee-profile.log. Active only with `--features profile`.
#[cfg(feature = "profile")]
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
    /// Per-process GPU utilisation. `None` means no data source is available —
    /// macOS exposes no per-process GPU accounting through sysinfo, and the
    /// `gpu_power` sampler only reports system-wide residency. Never fake this
    /// with a heuristic; the UI renders `None` as a dash, not as 0%.
    pub gpu_usage: Option<f32>,
    pub memory: u64,
    pub ports: Vec<PortInfo>,
    /// Which kind of dev tool this is, if any. `None` means the process is not
    /// a dev tool and stays out of the tier at the top of the table.
    pub category: Option<Category>,
    pub cwd: Option<String>,
    pub exe: Option<String>,
    pub run_time: u64,
    pub thread_count: u32,
}

#[derive(Debug, Clone)]
pub struct ProcessDetails {
    pub pid: u32,
    pub fd_count: Option<u32>,
    pub thread_count_macos: Option<u32>,
}

pub fn fetch_process_details(pid: u32) -> ProcessDetails {
    let fd_count = fetch_fd_count(pid);
    let thread_count_macos = fetch_thread_count_macos(pid);
    ProcessDetails {
        pid,
        fd_count,
        thread_count_macos,
    }
}

#[cfg(target_os = "linux")]
fn fetch_fd_count(pid: u32) -> Option<u32> {
    std::fs::read_dir(format!("/proc/{}/fd", pid))
        .ok()
        .map(|d| d.count() as u32)
}

#[cfg(target_os = "macos")]
fn fetch_fd_count(pid: u32) -> Option<u32> {
    let output = Command::new("lsof")
        .args(["-p", &pid.to_string(), "-nP"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let count = String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .count();
    Some(u32::try_from(count).unwrap_or(u32::MAX))
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn fetch_fd_count(_pid: u32) -> Option<u32> {
    None
}

#[cfg(target_os = "macos")]
fn fetch_thread_count_macos(pid: u32) -> Option<u32> {
    let output = Command::new("ps")
        .args(["-M", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let count = String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .count();
    Some(u32::try_from(count).unwrap_or(u32::MAX))
}

#[cfg(not(target_os = "macos"))]
fn fetch_thread_count_macos(_pid: u32) -> Option<u32> {
    None
}

#[derive(Debug, Clone, Copy)]
pub enum SortMode {
    Cpu,
    Memory,
    Name,
    Pid,
}

impl SortMode {
    pub const fn next(self) -> Self {
        match self {
            Self::Cpu => Self::Memory,
            Self::Memory => Self::Name,
            Self::Name => Self::Pid,
            Self::Pid => Self::Cpu,
        }
    }
}

impl Protocol {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "TCP" => Some(Self::Tcp),
            "UDP" => Some(Self::Udp),
            _ => None,
        }
    }
}

impl ConnectionState {
    fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "LISTEN" => Self::Listen,
            "ESTABLISHED" => Self::Established,
            _ => Self::Other,
        }
    }
}

fn get_process_ports() -> HashMap<u32, Vec<PortInfo>> {
    let mut port_map = HashMap::new();

    // Run lsof command to get network connections
    #[cfg(feature = "profile")]
    let lsof_start = Instant::now();
    // lsof not available or failed
    let Ok(output) = Command::new("lsof").args(["-i", "-P", "-n"]).output() else {
        return port_map;
    };
    #[cfg(feature = "profile")]
    log_timing("lsof_command", lsof_start.elapsed().as_millis());

    if !output.status.success() {
        return port_map;
    }

    #[cfg(feature = "profile")]
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
    #[cfg(feature = "profile")]
    log_timing("lsof_parse", parse_start.elapsed().as_millis());

    port_map
}

fn parse_lsof_line(line: &str) -> Option<(u32, PortInfo)> {
    // Parse lines like:
    // rapportd   1000 adam    8u  IPv4 0xe349afbd3b2ee8ee      0t0  TCP *:60744 (LISTEN)
    // identitys  1016 adam   18u  IPv4 0x34f005a6e91ac63b      0t0  UDP *:*

    let parts: Vec<&str> = line.split_whitespace().collect();

    // Extract PID (second column, index 1)
    let pid = parts.get(1)?.parse::<u32>().ok()?;

    // Extract protocol (8th column, index 7: TCP or UDP)
    let protocol = Protocol::from_str(parts.get(7)?)?;

    // Extract address info (9th column, index 8)
    let addr_part = *parts.get(8)?;

    // Skip non-port entries like "*:*"
    if addr_part == "*:*" {
        return None;
    }

    // Extract state if present (in parentheses at the end)
    let state = parts
        .get(9)
        .and_then(|s| s.strip_prefix('('))
        .and_then(|s| s.strip_suffix(')'))
        .map_or(ConnectionState::Other, ConnectionState::from_str);

    // Parse the address part
    let (local_addr, remote_addr) = match addr_part.split_once("->") {
        // Connection: local->remote
        Some((local, remote)) => (Some(local.to_string()), Some(remote.to_string())),
        // Listening or single address
        None => (Some(addr_part.to_string()), None),
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

    let (_, port_str) = addr.rsplit_once(':')?;
    port_str.parse().ok()
}

/// Resolve a process's UID to a username, caching the lookup: the fallback path
/// is an FFI call, and the same handful of UIDs recur on every refresh.
fn resolve_user(
    uid_cache: &mut HashMap<u32, String>,
    users: &Users,
    process: &sysinfo::Process,
) -> String {
    let Some(uid) = process.user_id() else {
        return "unknown".to_string();
    };
    let uid_value = **uid;
    if let Some(cached) = uid_cache.get(&uid_value) {
        return cached.clone();
    }
    // sysinfo's user database first, then libc for system users it omits, then
    // the numeric UID rather than nothing.
    let username = users.get_user_by_id(uid).map_or_else(
        || get_username_from_uid(uid_value).unwrap_or_else(|| uid_value.to_string()),
        |user| user.name().to_string(),
    );
    uid_cache.insert(uid_value, username.clone());
    username
}

#[derive(Debug)]
pub struct ProcessMonitor {
    system: System,
    users: Users,
    processes: Vec<ProcessInfo>,
    sort_mode: SortMode,
    /// Cache UID -> username mappings to avoid repeated FFI calls
    uid_cache: HashMap<u32, String>,
    /// What the last lsof scan saw, keyed by pid: the start time of the process
    /// it saw there, and the ports it found (empty for processes with none).
    /// Reused on refreshes that skip lsof so ports don't blink out of the table
    /// between scans. The start time is the process's identity: a pid recycled
    /// since the scan fails the match, so it inherits no ports and counts as
    /// unscanned, which brings the next scan forward.
    last_scan: HashMap<u32, ScannedProcess>,
}

/// One process as the last lsof scan saw it.
#[derive(Debug, Clone)]
struct ScannedProcess {
    /// Process start time, in seconds since the epoch. Distinguishes a live pid
    /// from a dead one whose number has been handed to something else.
    start_time: u64,
    ports: Vec<PortInfo>,
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

        Self {
            system,
            users,
            processes: Vec::new(),
            sort_mode: SortMode::Name,
            uid_cache: HashMap::new(),
            last_scan: HashMap::new(),
        }
    }

    /// Run lsof and record what it saw for every live process, ports or not.
    fn rescan_ports(&mut self) {
        let mut found = get_process_ports();
        self.last_scan = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| {
                let pid = pid.as_u32();
                let scanned = ScannedProcess {
                    start_time: process.start_time(),
                    ports: found.remove(&pid).unwrap_or_default(),
                };
                (pid, scanned)
            })
            .collect();
    }

    /// Refresh process information.
    /// - `include_ports`: Whether to run lsof to get port information (expensive)
    /// - `full_refresh`: If true, refresh memory/user/cmd info; if false, only refresh CPU usage
    ///
    /// Returns true when a live process was not covered by the last port scan,
    /// so the caller can bring the next scan forward.
    pub fn refresh(&mut self, include_ports: bool, full_refresh: bool) -> bool {
        // Refresh process information
        #[cfg(feature = "profile")]
        let sysinfo_start = Instant::now();

        // CPU-only refresh is faster; full refresh includes memory and user info
        let refresh_kind = if full_refresh {
            ProcessRefreshKind::new()
                .with_cpu()
                .with_memory()
                .with_user(UpdateKind::Always)
                .with_cmd(UpdateKind::OnlyIfNotSet)
        } else {
            // CPU-only refresh - much lighter weight
            ProcessRefreshKind::new().with_cpu()
        };

        self.system
            .refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind);
        #[cfg(feature = "profile")]
        log_timing(
            if full_refresh {
                "sysinfo_refresh_full"
            } else {
                "sysinfo_refresh_cpu"
            },
            sysinfo_start.elapsed().as_millis(),
        );

        if include_ports {
            self.rescan_ports();
        }

        // A process the last scan never saw, or one that has taken over a pid
        // since, has unknown ports. Tell the caller so it can rescan sooner.
        let unscanned = self.system.processes().iter().any(|(pid, process)| {
            self.last_scan
                .get(&pid.as_u32())
                .is_none_or(|scanned| scanned.start_time != process.start_time())
        });

        // Hoisted out of the closure below so it borrows only this field,
        // leaving `uid_cache` free to be borrowed mutably alongside it.
        let last_scan = &self.last_scan;
        let ports_for = |pid: u32, start_time: u64| {
            last_scan
                .get(&pid)
                .filter(|scanned| scanned.start_time == start_time)
                .map_or_else(Vec::new, |scanned| scanned.ports.clone())
        };

        // On cpu-only refreshes we can reuse the previously-built ProcessInfo
        // for each pid and just mutate its CPU/GPU fields. This skips the
        // per-process string allocations for name/cmd/user/cwd/exe.
        let mut prev: HashMap<u32, ProcessInfo> = if full_refresh {
            HashMap::new()
        } else {
            std::mem::take(&mut self.processes)
                .into_iter()
                .map(|p| (p.pid, p))
                .collect()
        };

        // Convert to our ProcessInfo format
        self.processes = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| {
                let process_pid = pid.as_u32();

                // Cheap path: existing entry with refreshed CPU only. New pids
                // discovered between full refreshes fall through to the full
                // build below so they get a complete record.
                if !full_refresh && let Some(mut existing) = prev.remove(&process_pid) {
                    existing.cpu_usage = process.cpu_usage();
                    existing.ports = ports_for(process_pid, process.start_time());
                    return existing;
                }

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

                // Classified here rather than in the UI: the CPU-only refresh
                // reuses the whole ProcessInfo, so this runs on the 10-second
                // full refresh and for new pids, not on every 2-second tick.
                let category = classify(&name, &cmd);

                let user = resolve_user(&mut self.uid_cache, &self.users, process);

                let ports = ports_for(process_pid, process.start_time());

                let cwd = process.cwd().map(|p| p.to_string_lossy().into_owned());
                let exe = process.exe().map(|p| p.to_string_lossy().into_owned());
                let run_time = process.run_time();
                let thread_count = process
                    .tasks()
                    .map_or(0, |t| u32::try_from(t.len()).unwrap_or(u32::MAX));

                ProcessInfo {
                    pid: process_pid,
                    name,
                    cmd,
                    user,
                    cpu_usage: process.cpu_usage(),
                    gpu_usage: None,
                    memory: process.memory(),
                    ports,
                    category,
                    cwd,
                    exe,
                    run_time,
                    thread_count,
                }
            })
            .collect();

        // Sort by current sort mode
        self.sort_processes();

        // // Limit to top 300 processes for performance
        // self.processes.truncate(300);

        unscanned
    }

    fn sort_processes(&mut self) {
        match self.sort_mode {
            SortMode::Cpu => {
                // total_cmp, not partial_cmp: a NaN cpu_usage would panic on unwrap.
                self.processes
                    .sort_by(|a, b| b.cpu_usage.total_cmp(&a.cpu_usage));
            }
            SortMode::Memory => {
                self.processes.sort_by_key(|p| std::cmp::Reverse(p.memory));
            }
            SortMode::Name => {
                // By cmd, not name: the column is labelled COMMAND and displays
                // cmd, so sorting by name filed every Node process under "node".
                self.processes.sort_by(|a, b| a.cmd.cmp(&b.cmd));
            }
            SortMode::Pid => {
                self.processes.sort_by_key(|p| p.pid);
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
            &raw mut pwd,
            buf.as_mut_ptr().cast::<libc::c_char>(),
            buf.len(),
            &raw mut result,
        );

        if ret == 0 && !result.is_null() {
            let username_ptr = (*result).pw_name;
            if !username_ptr.is_null() {
                let username = CStr::from_ptr(username_ptr);
                return username.to_str().ok().map(std::string::ToString::to_string);
            }
        }
        None
    }
}

#[cfg(not(target_os = "macos"))]
fn get_username_from_uid(_uid: u32) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use sysinfo::Pid;

    fn process_named(pid: u32, name: &str, cmd: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            name: name.to_string(),
            cmd: cmd.to_string(),
            user: "adam".to_string(),
            cpu_usage: 0.0,
            gpu_usage: None,
            memory: 0,
            ports: Vec::new(),
            category: crate::category::classify(name, cmd),
            cwd: None,
            exe: None,
            run_time: 0,
            thread_count: 0,
        }
    }

    /// The column is labelled COMMAND and displays `cmd`, so the sort must order
    /// by `cmd`. Sorting by `name` put every Node process under "node" while the
    /// table showed "next dev", "vite" and "claude".
    #[test]
    fn command_sort_orders_by_cmd_not_name() {
        let mut monitor = ProcessMonitor::new();
        monitor.sort_mode = SortMode::Name;
        monitor.processes = vec![
            process_named(1, "node", "vite"),
            process_named(2, "node", "claude"),
            process_named(3, "alpha", "next dev"),
        ];

        monitor.sort_processes();

        let order: Vec<&str> = monitor.processes.iter().map(|p| p.cmd.as_str()).collect();
        assert_eq!(order, vec!["claude", "next dev", "vite"]);
    }

    /// The collector defaults to the COMMAND sort, which is the only mode the
    /// dev tier groups under.
    #[test]
    fn command_is_the_default_sort() {
        let monitor = ProcessMonitor::new();
        assert!(matches!(monitor.sort_mode, SortMode::Name));
    }

    fn listening_on(port: u16) -> PortInfo {
        PortInfo {
            port,
            protocol: Protocol::Tcp,
            state: ConnectionState::Listen,
            local_address: Some(format!("*:{port}")),
            remote_address: None,
        }
    }

    /// Start time of this test process, as sysinfo reports it.
    fn own_start_time(monitor: &ProcessMonitor) -> u64 {
        monitor
            .system
            .process(Pid::from_u32(std::process::id()))
            .expect("test process should be in the process list")
            .start_time()
    }

    fn seed_scan(monitor: &mut ProcessMonitor, start_time: u64, ports: Vec<PortInfo>) {
        monitor
            .last_scan
            .insert(std::process::id(), ScannedProcess { start_time, ports });
    }

    fn own_ports(monitor: &ProcessMonitor) -> Vec<u16> {
        monitor
            .get_processes()
            .iter()
            .find(|p| p.pid == std::process::id())
            .expect("test process should be in the process list")
            .ports
            .iter()
            .map(|p| p.port)
            .collect()
    }

    /// Refreshes that skip lsof used to rebuild every process with no ports,
    /// so a listening port blinked out of the table between scans.
    #[test]
    fn ports_survive_a_refresh_that_skips_lsof() {
        let mut monitor = ProcessMonitor::new();
        let started = own_start_time(&monitor);

        for (include_ports, full_refresh) in [(false, true), (false, false)] {
            seed_scan(&mut monitor, started, vec![listening_on(4242)]);
            monitor.refresh(include_ports, full_refresh);
            assert_eq!(
                own_ports(&monitor),
                vec![4242],
                "refresh({include_ports}, {full_refresh}) dropped cached ports"
            );
        }
    }

    /// Pids are recycled. Serving the previous occupant's ports would attribute
    /// a port to a process that never opened it, which is worse than showing
    /// none until the next scan.
    #[test]
    fn a_recycled_pid_does_not_inherit_the_previous_process_ports() {
        let mut monitor = ProcessMonitor::new();
        let started = own_start_time(&monitor);
        seed_scan(&mut monitor, started - 1, vec![listening_on(4242)]);

        let unscanned = monitor.refresh(false, true);

        assert!(own_ports(&monitor).is_empty(), "stale ports were served");
        assert!(
            unscanned,
            "a pid whose start time moved must be reported as unscanned"
        );
    }

    /// A process the last scan never saw has unknown ports, so the caller is
    /// told to bring the next scan forward.
    #[test]
    fn unscanned_pids_are_reported_to_the_caller() {
        let mut monitor = ProcessMonitor::new();
        assert!(
            monitor.refresh(false, true),
            "no scan has run yet, so every live pid is unscanned"
        );

        monitor.last_scan = monitor
            .system
            .processes()
            .iter()
            .map(|(pid, process)| {
                (
                    pid.as_u32(),
                    ScannedProcess {
                        start_time: process.start_time(),
                        ports: Vec::new(),
                    },
                )
            })
            .collect();
        assert!(
            !monitor.refresh(false, false),
            "every live pid was covered by the last scan"
        );
    }
}
