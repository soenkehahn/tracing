// this file is taken from git@github.com:luser/tracetree.git,
// commit 587eaaa90ad2469b37a6a8568e024276e99e11dc,
// under the license in ./tracetree.rs.license

extern crate indextree;
extern crate libc;
extern crate nix;
extern crate serde;
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
use serde::ser::{SerializeSeq, SerializeStruct};
use serde::{Serialize, Serializer};
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
    pub cmdline: Vec<String>,
}

impl Default for ProcessInfo {
    fn default() -> ProcessInfo {
        ProcessInfo {
            pid: 0,
            ended: false,
            cmdline: vec![],
        }
    }
}

#[derive(Debug)]
pub struct ProcessTree {
    pub arena: Arena<ProcessInfo>,
    pub root: NodeId,
}

impl ProcessTree {
    pub fn spawn<T>(mut cmd: Command, cmdline: &[T]) -> AppResult<ProcessTree>
    where
        T: AsRef<str>,
    {
        let child = cmd.spawn_ptrace().chain_err(|| "Error spawning process")?;
        let pid = child.id() as pid_t;
        trace!("Spawned process {}", pid);
        ptrace_setoptions(
            pid,
            PTRACE_O_TRACEEXEC | PTRACE_O_TRACEFORK | PTRACE_O_TRACEVFORK | PTRACE_O_TRACECLONE,
        )
        .chain_err(|| "Error setting ptrace options")?;
        let mut arena = Arena::new();
        let mut pids = HashMap::new();
        let root = get_or_insert_pid(pid, &mut arena, &mut pids);
        arena[root].data.cmdline = cmdline.iter().map(|s| s.as_ref().to_string()).collect();
        continue_process(pid, None).chain_err(|| "Error continuing process")?;
        loop {
            if !root
                .descendants(&arena)
                .any(|node| arena[node].data.ended == false)
            {
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
                            .chain_err(|| "Failed to get pid of forked process")?;
                            let name = match event {
                                PTRACE_EVENT_FORK => "fork",
                                PTRACE_EVENT_VFORK => "vfork",
                                PTRACE_EVENT_CLONE => "clone",
                                _ => unreachable!(),
                            };
                            trace!("[{}] {} new process {}", pid, name, new_pid);
                            match pids.get(&pid) {
                                Some(&parent) => {
                                    let cmdline = {
                                        let parent_data = &arena[parent].data;
                                        if parent_data.cmdline.len() > 1 {
                                            parent_data.cmdline[..1].to_vec()
                                        } else {
                                            vec![]
                                        }
                                    };
                                    let child = get_or_insert_pid(new_pid, &mut arena, &mut pids);
                                    arena[child].data.cmdline = cmdline;
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
                                            arena[node].data.cmdline = cmdline;
                                            Ok(())
                                        })
                                        .chain_err(|| "Couldn't read cmdline")?;
                                }
                                None => bail(format!("Got an exec event for unknown pid {}", pid))?,
                            }
                        }
                        _ => panic!("Unexpected ptrace event: {:?}", event),
                    }
                    continue_process(pid, None).chain_err(|| "Error continuing process")?;
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
                    continue_process(pid, continue_sig).chain_err(|| "Error continuing process")?;
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

    pub fn get_descendants(self) -> Vec<String> {
        self.root
            .descendants(&self.arena)
            .map(|node_id: NodeId| self.arena.get(node_id).unwrap().data.cmdline.clone())
            .flatten()
            .collect()
    }
}

struct ProcessInfoSerializable<'a>(NodeId, &'a Arena<ProcessInfo>);
struct ChildrenSerializable<'a>(NodeId, &'a Arena<ProcessInfo>);

impl<'a> Serialize for ProcessInfoSerializable<'a> {
    fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("ProcessInfo", 5)?;
        {
            let info = &self.1[self.0].data;
            state.serialize_field("pid", &info.pid)?;
            state.serialize_field("ended", &info.ended)?;
            state.serialize_field("cmdline", &info.cmdline)?;
        }
        state.serialize_field("children", &ChildrenSerializable(self.0, self.1))?;
        state.end()
    }
}

impl<'a> Serialize for ChildrenSerializable<'a> {
    fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let len = self.0.children(self.1).count();
        let mut seq = serializer.serialize_seq(Some(len))?;
        for c in self.0.children(self.1) {
            seq.serialize_element(&ProcessInfoSerializable(c, self.1))?;
        }
        seq.end()
    }
}

impl Serialize for ProcessTree {
    fn serialize<S>(&self, serializer: S) -> ::std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let root_pi = ProcessInfoSerializable(self.root, &self.arena);
        root_pi.serialize(serializer)
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
