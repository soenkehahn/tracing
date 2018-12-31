// this file is taken from git@github.com:luser/tracetree.git,
// commit 587eaaa90ad2469b37a6a8568e024276e99e11dc,
// under the license in ./tracetree.rs.license

extern crate indextree;
extern crate libc;
extern crate nix;
extern crate spawn_ptrace;

use crate::error::{bail, AppResult, ChainErr};
pub use indextree::NodeEdge;
use indextree::{Arena, NodeId};
use libc::{c_long, pid_t};
use nix::c_void;
use nix::sys::ptrace::ptrace::{
    PTRACE_CONT, PTRACE_GETEVENTMSG, PTRACE_O_TRACECLONE, PTRACE_O_TRACEEXEC, PTRACE_O_TRACEFORK,
    PTRACE_O_TRACEVFORK,
};
use nix::sys::ptrace::ptrace::{
    PTRACE_EVENT_CLONE, PTRACE_EVENT_EXEC, PTRACE_EVENT_FORK, PTRACE_EVENT_VFORK,
};
use nix::sys::ptrace::{ptrace, ptrace_setoptions};
use nix::sys::signal;
use nix::sys::wait::{waitpid, WaitStatus};
use spawn_ptrace::CommandPtraceSpawn;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::process::Command;
use std::ptr;

#[derive(Debug)]
pub struct ProcessInfo {
    pub pid: pid_t,
    pub ended: bool,
    pub process_child: Option<ProcessChild>,
}

impl Default for ProcessInfo {
    fn default() -> ProcessInfo {
        ProcessInfo {
            pid: 0,
            ended: false,
            process_child: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessChild {
    pub executable: String,
    pub arguments: Vec<String>,
}

impl ProcessChild {
    fn from_cmdline(input: Vec<String>) -> Option<ProcessChild> {
        input.split_first().map(|(head, tail)| ProcessChild {
            executable: head.to_string(),
            arguments: tail.to_vec(),
        })
    }
}

#[derive(Debug)]
pub struct ProcessTree {
    pub arena: Arena<ProcessInfo>,
    pub root: NodeId,
}

impl ProcessTree {
    pub fn spawn(mut cmd: Command) -> AppResult<ProcessTree> {
        let child = cmd.spawn_ptrace().chain_err("Error spawning process")?;
        let pid = child.id() as pid_t;
        trace!("Spawned process {}", pid);
        ptrace_setoptions(
            pid,
            PTRACE_O_TRACEEXEC | PTRACE_O_TRACEFORK | PTRACE_O_TRACEVFORK | PTRACE_O_TRACECLONE,
        )
        .chain_err("Error setting ptrace options")?;
        let mut arena = Arena::new();
        let mut pids = HashMap::new();
        let root = get_or_insert_pid(pid, &mut arena, &mut pids);
        continue_process(pid, None).chain_err("Error continuing process")?;
        loop {
            if !root.descendants(&arena).any(|node| !arena[node].data.ended) {
                break;
            }
            match waitpid(-1, None) {
                Ok(WaitStatus::Exited(pid, ret)) => {
                    trace!("Process {} exited with status {}", pid, ret);
                    let node = get_or_insert_pid(pid, &mut arena, &mut pids);
                    arena[node].data.ended = true;
                }
                Ok(WaitStatus::Signaled(pid, sig, _)) => {
                    trace!("Process {} exited with signal {:?}", pid, sig);
                    let node = get_or_insert_pid(pid, &mut arena, &mut pids);
                    arena[node].data.ended = true;
                }
                Ok(WaitStatus::PtraceEvent(pid, _sig, event)) => {
                    match event {
                        PTRACE_EVENT_FORK | PTRACE_EVENT_VFORK | PTRACE_EVENT_CLONE => {
                            let mut new_pid: pid_t = 0;
                            ptrace(
                                PTRACE_GETEVENTMSG,
                                pid,
                                ptr::null_mut(),
                                &mut new_pid as *mut pid_t as *mut c_void,
                            )
                            .chain_err("Failed to get pid of forked process")?;
                            let name = match event {
                                PTRACE_EVENT_FORK => "fork",
                                PTRACE_EVENT_VFORK => "vfork",
                                PTRACE_EVENT_CLONE => "clone",
                                _ => unreachable!(),
                            };
                            trace!("[{}] {} new process {}", pid, name, new_pid);
                            match pids.get(&pid) {
                                Some(&parent) => {
                                    let child = get_or_insert_pid(new_pid, &mut arena, &mut pids);
                                    parent.append(child, &mut arena);
                                }
                                None => bail(format!(
                                    "Got an {:?} event for unknown parent pid {}",
                                    event, pid
                                ))?,
                            }
                        }
                        PTRACE_EVENT_EXEC => {
                            let mut buf = vec![];
                            match pids.get(&pid) {
                                Some(&node) => {
                                    File::open(format!("/proc/{}/cmdline", pid))
                                        .and_then(|mut f| f.read_to_end(&mut buf))
                                        .and_then(|_| {
                                            let mut cmdline = buf
                                                .split(|&b| b == 0)
                                                .map(|bytes| {
                                                    String::from_utf8_lossy(bytes).into_owned()
                                                })
                                                .collect::<Vec<_>>();
                                            cmdline.pop();
                                            debug!("[{}] exec {:?}", pid, cmdline);
                                            arena[node].data.process_child =
                                                ProcessChild::from_cmdline(cmdline);
                                            Ok(())
                                        })
                                        .chain_err("Couldn't read cmdline")?;
                                }
                                None => bail(format!("Got an exec event for unknown pid {}", pid))?,
                            }
                        }
                        _ => panic!("Unexpected ptrace event: {:?}", event),
                    }
                    continue_process(pid, None).chain_err("Error continuing process")?;
                }
                Ok(WaitStatus::Stopped(pid, sig)) => {
                    trace!("[{}] stopped with {:?}", pid, sig);
                    // Sometimes we get the SIGSTOP+exit from a child before we get the clone
                    // stop from the parent, so insert any unknown pids here so we have a better
                    // approximation of the process start time.
                    get_or_insert_pid(pid, &mut arena, &mut pids);
                    let continue_sig = if sig == signal::Signal::SIGSTOP {
                        None
                    } else {
                        Some(sig)
                    };
                    continue_process(pid, continue_sig).chain_err("Error continuing process")?;
                }
                Ok(s) => bail(format!("Unexpected process status: {:?}", s))?,
                Err(e) => {
                    match e {
                        nix::Error::Sys(nix::Errno::EINTR) => {
                            /*FIXME
                            if SIGNAL_DELIVERED.swap(false, Ordering::Relaxed) {
                                println!("Active processes:");
                                print_process_tree(root, arena, |info| info.ended.is_none());
                            }
                             */
                        }
                        _ => bail(format!("ptrace error: {:?}", e))?,
                    }
                }
            }
        }
        Ok(ProcessTree { arena, root })
    }

    pub fn get_descendants(self) -> Vec<ProcessChild> {
        self.root
            .descendants(&self.arena)
            .map(|node_id: NodeId| self.arena.get(node_id).unwrap().data.process_child.clone())
            .filter_map(|x| x)
            .collect()
    }
}

fn get_or_insert_pid(
    pid: pid_t,
    arena: &mut Arena<ProcessInfo>,
    map: &mut HashMap<pid_t, NodeId>,
) -> NodeId {
    *map.entry(pid).or_insert_with(|| {
        arena.new_node(ProcessInfo {
            pid,
            ..ProcessInfo::default()
        })
    })
}

fn continue_process(pid: pid_t, signal: Option<signal::Signal>) -> nix::Result<c_long> {
    let data = signal
        .map(|s| s as i32 as *mut c_void)
        .unwrap_or(ptr::null_mut());
    ptrace(PTRACE_CONT, pid, ptr::null_mut(), data)
}
