use std::{
    collections::{HashMap, HashSet},
    num::ParseIntError,
    path::{Path, PathBuf},
};

use once_cell::sync::Lazy;
use regex::Regex;

mod nom_parse;

mod file_tree;

#[derive(Debug)]
pub struct StraceEvent<'a> {
    syscall: &'a str,
    args: Vec<&'a str>,
    result: &'a str,
}

fn parse_into_event<'a>(input: &'a str) -> Result<StraceEvent<'a>, ()> {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(\w+)\((.*)\)\s+= (.+)"#).unwrap());
    //static ARGS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(.*),?(.+)?"#).unwrap());

    let data = RE.captures(input).ok_or(())?;


    //let args_list = args_re.captures(data.get(2).unwrap().as_str()).ok_or(())?;
    //let mut args = vec![];
    //for arg in args_list.iter().skip(1) {
    //match arg {
    //Some(s) => args.push(s.as_str()),
    //None => {}
    //}
    //}

    let args = data.get(2).unwrap().as_str().split(", ").collect();

    Ok(StraceEvent {
        syscall: data.get(1).unwrap().as_str(),
        args,
        result: data.get(3).unwrap().as_str(),
    })
}

#[derive(Debug)]
struct Eventer<'a> {
    syscalls: HashSet<&'a str>,
    open_fds: HashMap<usize, FDInfo<'a>>,
    closed_fds: Vec<FDInfo<'a>>,
}

#[derive(Debug)]
pub enum FDInfo<'a> {
    File { path_buf: PathBuf, flags: &'a str },
    Stdio { name: &'a str },
    Pipe { flags: &'a str },
    Socket,
    Other,
}

impl<'a> FDInfo<'a> {
    const STDIN: Self = Self::Stdio { name: "stdin" };
    const STDOUT: Self = Self::Stdio { name: "stdout" };
    const STDERR: Self = Self::Stdio { name: "stderr" };

    fn new_file(name: &'a str, flags: &'a str) -> Self {
        let name = name.trim_matches('"');
        let path_buf = Path::new(&name).canonicalize().unwrap();
        FDInfo::File { path_buf, flags }
    }

    fn name(&'a self) -> &'a str {
        match self {
            Self::File { path_buf, .. } => path_buf.as_os_str().to_str().unwrap(),
            Self::Stdio { name } => name,
            Self::Pipe { .. } => "(pipe)",
            Self::Socket => "(socket)",
            Self::Other => "(unknown)",
        }
    }
}

use thiserror::Error;

use crate::file_tree::FileTree;
#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("Failed to parse result {0}")]
    BadResult(#[from] ParseIntError),
    #[error("Not enough args. Needed {0}, got {1}")]
    TooShort(usize, usize),
    #[error("Tried to modify an fd we weren't holding, {0}")]
    UseEmptyFD(usize),
    #[error("Overwrote an FD slot, {0}")]
    OverwriteFD(usize),
}

enum EventType {
    Read,
    Write,
}

impl ProcessError {
    fn check_len<T>(req: usize, args: &[T]) -> Result<(), ProcessError> {
        let len = args.len();

        if len < req {
            Err(ProcessError::TooShort(req, len))
        } else {
            Ok(())
        }
    }
}

impl<'a> Eventer<'a> {
    fn new() -> Self {
        Self {
            syscalls: Default::default(),
            open_fds: vec![(0, FDInfo::STDIN), (1, FDInfo::STDOUT), (2, FDInfo::STDERR)]
                .into_iter()
                .collect(),
            closed_fds: Default::default(),
        }
    }

    fn process(&mut self, ev: StraceEvent<'a>) -> Result<(), ProcessError> {
        self.syscalls.insert(ev.syscall);

        match ev.syscall {
            "openat" => {
                ProcessError::check_len(3, &ev.args)?;
                let relflag = ev.args[0];
                let file = ev.args[1];
                let flags = ev.args[2];

                if relflag != "AT_FDCWD" {
                    panic!("openat unseen relflag");
                }

                let fd = ev.result;

                if !fd.starts_with("-") {
                    self.new_fd(fd.parse()?, FDInfo::new_file(file, flags))?
                }
            }
            "open" => {
                ProcessError::check_len(3, &ev.args)?;
                let file = ev.args[0];
                let flags = ev.args[1];
                let fd = ev.result;

                if !fd.starts_with("-") {
                    self.new_fd(fd.parse()?, FDInfo::new_file(file, flags))?
                }
            }
            "close" => {
                ProcessError::check_len(1, &ev.args)?;
                let fd = ev.args[0].parse()?;
                let result: i64 = ev.result.parse()?;
                if result == 0 {
                    self.close_fd(fd)?;
                }
            }
            "read" => {
                ProcessError::check_len(1, &ev.args)?;
                let fd = ev.args[0].parse()?;
                self.fd_event(fd, EventType::Read)?;
            }
            "write" => {
                ProcessError::check_len(1, &ev.args)?;
                let fd = ev.args[0].parse()?;
                self.fd_event(fd, EventType::Write)?;
            }
            //TODOs
            "readlink" => {} //read
            "lstat" => {}    //read
            "statx" => {}    //read
            "linkat" => {}   //modify
            "unlink" => {}   //modify
            "pipe" | "pipe2" => {
                ProcessError::check_len(3, &ev.args)?;
                let read_end: usize = ev.args[0][1..].parse()?;
                let write_end: usize = ev.args[1][..1].parse()?;
                self.new_fd(read_end, FDInfo::Pipe { flags: "O_RDONLY" })?;
                self.new_fd(write_end, FDInfo::Pipe { flags: "O_WRONLY" })?;
            }
            "socket" => {
                ProcessError::check_len(3, &ev.args)?;
                let fd = ev.result.parse()?;
                self.new_fd(fd, FDInfo::Socket)?;
            }
            _ => {}
        };

        Ok(())
    }

    fn new_fd(&mut self, id: usize, info: FDInfo<'a>) -> Result<(), ProcessError> {
        //println!("Opening new fd {}: {:?}", id, name);
        println!("OPEN    {}", info.name());

        match self.open_fds.insert(id, info) {
            Some(file) => {
                self.closed_fds.push(file);
                Err(ProcessError::OverwriteFD(id))
            }
            None => Ok(()),
        }
    }

    fn close_fd(&mut self, id: usize) -> Result<(), ProcessError> {
        match self.open_fds.remove(&id) {
            Some(file) => {
                //println!("Closing fd {}: {}", id, file.name);
                println!("CLOSE   {}", file.name());
                self.closed_fds.push(file);
                Ok(())
            }
            None => Err(ProcessError::UseEmptyFD(id)),
        }
    }

    fn fd_event(&self, fd: usize, etype: EventType) -> Result<(), ProcessError> {
        let info = match self.open_fds.get(&fd) {
            Some(info) => info,
            None => return Err(ProcessError::UseEmptyFD(fd)),
        };

        match etype {
            EventType::Read => {
                println!("READ    {}", info.name());
                //println!("Reading from {:?}", &info);
            }
            EventType::Write => {
                println!("WRITE   {}", info.name());
                //println!("Writing to {:?}", &info);
            }
        };

        Ok(())
    }

    fn close_all_fds(&mut self) {
        for (_, v) in self.open_fds.drain() {
            self.closed_fds.push(v);
        }
    }

    fn print_tree(&mut self) {
        println!("{:?}", self.closed_fds);
        let ft = FileTree::new(self.closed_fds.drain(..));

        ft.print();

        //println!("{:?}", ft);
    }
}

fn main() {
    let mut args = std::env::args();
    if args.len() < 1 {
        println!("Not enough args. usage `cargo run [file]`");
    }
    let file = args.nth(1).unwrap();

    let file = std::fs::read_to_string(file).expect("couldn't open strace file");

    let mut eventer = Eventer::new();

    for (lnnum, line) in file.lines().enumerate() {
        if line == "" {
            continue;
        }
        if line.starts_with("+++") {
            continue;
        }
        if line.starts_with("---") {
            continue;
        }

        //let e = nom_parse::parse_into_event(line);
        let e = parse_into_event(line).unwrap();

        match eventer.process(e) {
            Ok(_) => {}
            Err(err) => {
                println!("got eventer error on line {}: {}", lnnum + 1, err);
            }
        }
    }

    eventer.close_all_fds();
    eventer.print_tree();
}

// strace -f -e trace=!write cargo build
// strace -f -s 1 cargo build
// strace -s 1 cargo build
//  strace -ff -o ./strace_all cargo build && cat strace_all.* > ./strace_all && rm strace_all.*
// rm -rf trace && mkdir trace && strace -ff -o ./trace/strace_all cargo build
