use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub owner_uid: u32,
    pub process_start_time_unix_ms: u128,
    pub canonical_executable_path: PathBuf,
    pub argv_fingerprint: Option<String>,
}

pub trait ProcessInspector: Send + Sync {
    fn inspect(&self, pid: u32) -> Result<Option<ProcessSnapshot>, String>;
    fn process_owns_port(&self, pid: u32, host: &str, port: u16) -> Result<bool, String>;
    fn current_uid(&self) -> u32;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemProcessInspector;

impl ProcessInspector for SystemProcessInspector {
    fn inspect(&self, pid: u32) -> Result<Option<ProcessSnapshot>, String> {
        if pid == 0 {
            return Ok(None);
        }
        #[cfg(target_os = "macos")]
        {
            macos::inspect(pid)
        }
        #[cfg(target_os = "linux")]
        {
            return linux::inspect(pid);
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = pid;
            Err("Bu platformda güvenli süreç incelemesi desteklenmiyor.".to_string())
        }
    }

    fn process_owns_port(&self, pid: u32, host: &str, port: u16) -> Result<bool, String> {
        if pid == 0 {
            return Ok(false);
        }
        #[cfg(target_os = "macos")]
        {
            macos::process_owns_port(pid, host, port)
        }
        #[cfg(target_os = "linux")]
        {
            return linux::process_owns_port(pid, host, port);
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = (pid, host, port);
            Err("Bu platformda port sahipliği incelemesi desteklenmiyor.".to_string())
        }
    }

    fn current_uid(&self) -> u32 {
        #[cfg(unix)]
        {
            // SAFETY: getuid has no preconditions and does not mutate memory.
            unsafe { libc::getuid() as u32 }
        }
        #[cfg(not(unix))]
        {
            0
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::ProcessSnapshot;
    use std::ffi::OsString;
    use std::mem::MaybeUninit;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;
    use std::process::Command;

    #[repr(C)]
    struct ProcBsdInfo {
        pbi_flags: u32,
        pbi_status: u32,
        pbi_xstatus: u32,
        pbi_pid: u32,
        pbi_ppid: u32,
        pbi_uid: u32,
        pbi_gid: u32,
        pbi_ruid: u32,
        pbi_rgid: u32,
        pbi_svuid: u32,
        pbi_svgid: u32,
        rfu_1: u32,
        pbi_comm: [i8; 16],
        pbi_name: [i8; 32],
        pbi_nfiles: u32,
        pbi_pgid: u32,
        pbi_pjobc: u32,
        e_tdev: u32,
        e_tpgid: u32,
        pbi_nice: i32,
        pbi_start_tvsec: u64,
        pbi_start_tvusec: u64,
    }

    #[link(name = "proc")]
    unsafe extern "C" {
        fn proc_pidinfo(pid: i32, flavor: i32, arg: u64, buffer: *mut u8, buffersize: i32) -> i32;
        fn proc_pidpath(pid: i32, buffer: *mut u8, buffersize: u32) -> i32;
    }

    const PROC_PIDTBSDINFO: i32 = 3;
    const PROC_PIDPATHINFO_MAXSIZE: usize = 4096;

    pub fn inspect(pid: u32) -> Result<Option<ProcessSnapshot>, String> {
        if pid == 0 || pid > i32::MAX as u32 {
            return Ok(None);
        }
        let mut info = MaybeUninit::<ProcBsdInfo>::zeroed();
        // SAFETY: info points to a writable buffer of the exact C structure size.
        let info_len = unsafe {
            proc_pidinfo(
                pid as i32,
                PROC_PIDTBSDINFO,
                0,
                info.as_mut_ptr().cast::<u8>(),
                std::mem::size_of::<ProcBsdInfo>() as i32,
            )
        };
        if info_len != std::mem::size_of::<ProcBsdInfo>() as i32 {
            let errno = std::io::Error::last_os_error();
            if errno.raw_os_error() == Some(libc::ESRCH) {
                return Ok(None);
            }
            return Err(format!("proc_pidinfo failed for pid {pid}: {errno}"));
        }
        // SAFETY: proc_pidinfo filled the structure after returning its exact size.
        let info = unsafe { info.assume_init() };
        let mut path = vec![0_u8; PROC_PIDPATHINFO_MAXSIZE];
        // SAFETY: path is a valid writable buffer owned by this function.
        let path_len =
            unsafe { proc_pidpath(pid as i32, path.as_mut_ptr().cast(), path.len() as u32) };
        if path_len <= 0 {
            return Err(format!("proc_pidpath failed for pid {pid}"));
        }
        path.truncate(path_len as usize);
        let executable = std::fs::canonicalize(PathBuf::from(OsString::from_vec(path)))
            .map_err(|error| format!("canonicalizing pid {pid} executable failed: {error}"))?;
        let argv_fingerprint = supplemental_argv_fingerprint(pid);
        Ok(Some(ProcessSnapshot {
            pid,
            owner_uid: info.pbi_uid,
            process_start_time_unix_ms: (info.pbi_start_tvsec as u128) * 1000
                + (info.pbi_start_tvusec as u128) / 1000,
            canonical_executable_path: executable,
            argv_fingerprint,
        }))
    }

    pub fn process_owns_port(pid: u32, _host: &str, port: u16) -> Result<bool, String> {
        // libproc supplies the authoritative process identity above. lsof is
        // only a supplemental socket ownership signal; it is never sufficient
        // to authorize a signal by itself.
        let output = Command::new("/usr/sbin/lsof")
            .args([
                "-nP",
                "-a",
                "-p",
                &pid.to_string(),
                &format!("-iTCP:{port}"),
                "-sTCP:LISTEN",
                "-F",
                "p",
            ])
            .output()
            .map_err(|error| format!("port ownership inspection failed: {error}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.lines().any(|line| line == format!("p{pid}")))
    }

    fn supplemental_argv_fingerprint(pid: u32) -> Option<String> {
        let output = Command::new("/bin/ps")
            .args(["-ww", "-o", "command=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let command = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if command.is_empty() {
            None
        } else {
            Some(crate::platform::process_inspector::fingerprint(&[command]))
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{fingerprint, ProcessSnapshot};
    use std::os::unix::fs::MetadataExt;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn inspect(pid: u32) -> Result<Option<ProcessSnapshot>, String> {
        let proc_dir = PathBuf::from("/proc").join(pid.to_string());
        if !proc_dir.exists() {
            return Ok(None);
        }
        let executable = std::fs::canonicalize(proc_dir.join("exe"))
            .map_err(|error| format!("canonicalizing process executable failed: {error}"))?;
        let metadata = std::fs::metadata(&proc_dir)
            .map_err(|error| format!("reading process metadata failed: {error}"))?;
        let owner_uid = metadata.uid();
        let stat = std::fs::read_to_string(proc_dir.join("stat"))
            .map_err(|error| format!("reading process stat failed: {error}"))?;
        let command_end = stat
            .rfind(')')
            .ok_or_else(|| "process stat command field is malformed".to_string())?;
        let start_ticks = stat[command_end + 2..]
            .split_whitespace()
            .nth(19)
            .ok_or_else(|| "process stat start time is missing".to_string())?
            .parse::<u128>()
            .map_err(|error| format!("parsing process start time failed: {error}"))?;
        let clock_ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        if clock_ticks <= 0 {
            return Err("system clock tick rate is unavailable".to_string());
        }
        let uptime_seconds = std::fs::read_to_string("/proc/uptime")
            .map_err(|error| format!("reading system uptime failed: {error}"))?
            .split_whitespace()
            .next()
            .ok_or_else(|| "system uptime is missing".to_string())?
            .parse::<f64>()
            .map_err(|error| format!("parsing system uptime failed: {error}"))?;
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis();
        let boot_time_ms = now_ms.saturating_sub((uptime_seconds * 1000.0) as u128);
        let start =
            boot_time_ms.saturating_add(start_ticks.saturating_mul(1000) / clock_ticks as u128);
        Ok(Some(ProcessSnapshot {
            pid,
            owner_uid,
            process_start_time_unix_ms: start,
            canonical_executable_path: executable,
            argv_fingerprint: std::fs::read(proc_dir.join("cmdline"))
                .ok()
                .filter(|argv| !argv.is_empty())
                .map(|argv| fingerprint(&[String::from_utf8_lossy(&argv).to_string()])),
        }))
    }

    pub fn process_owns_port(pid: u32, _host: &str, port: u16) -> Result<bool, String> {
        let port_hex = format!("{port:04X}");
        let mut listening_inodes = Vec::new();
        for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
            let content = std::fs::read_to_string(table)
                .map_err(|error| format!("reading {table} failed: {error}"))?;
            for line in content.lines().skip(1) {
                let fields: Vec<&str> = line.split_whitespace().collect();
                if fields.len() > 9
                    && fields[1]
                        .rsplit(':')
                        .next()
                        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&port_hex))
                    && fields[3] == "0A"
                {
                    listening_inodes.push(fields[9].to_string());
                }
            }
        }
        if listening_inodes.is_empty() {
            return Ok(false);
        }
        let fd_dir = PathBuf::from("/proc").join(pid.to_string()).join("fd");
        for entry in std::fs::read_dir(fd_dir)
            .map_err(|error| format!("reading process file descriptors failed: {error}"))?
        {
            let link = std::fs::read_link(
                entry
                    .map_err(|error| format!("reading process file descriptor failed: {error}"))?
                    .path(),
            )
            .map_err(|error| format!("reading process file descriptor link failed: {error}"))?;
            let target = link.to_string_lossy();
            if listening_inodes
                .iter()
                .any(|inode| target == format!("socket:[{inode}]"))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

pub fn fingerprint(values: &[String]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    values.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
