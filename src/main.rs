use std::{
    collections::{HashMap, HashSet},
    num::ParseIntError,
};

use regex::Regex;

mod nom_parse;

#[derive(Debug)]
pub struct StraceEvent<'a> {
    syscall: &'a str,
    args: Vec<&'a str>,
    result: &'a str,
}

fn parse_into_event<'a>(
    re: &Regex,
    _args_re: &Regex,
    input: &'a str,
) -> Result<StraceEvent<'a>, ()> {
    let data = re.captures(input).ok_or(())?;

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
    open_fds: HashMap<usize, FileInfo<'a>>,
    seen_files: HashSet<&'a str>,
}

#[derive(Debug)]
struct FileInfo<'a> {
    name: &'a str,
    flags: &'a str,
}

impl<'a> FileInfo<'a> {
    const STDIN: Self = Self::new("(stdin)", "O_RDONLY");
    const STDOUT: Self = Self::new("(stdout)", "O_RDWR");
    const STDERR: Self = Self::new("(stderr)", "O_RDWR");

    const fn new(name: &'a str, flags: &'a str) -> Self {
        FileInfo { name, flags }
    }
}

use thiserror::Error;
#[derive(Error, Debug)]
pub enum ProcessError {
    #[error("Failed to parse result {0}")]
    BadResult(#[from] ParseIntError),
    #[error("Not enough args. Needed {0}, got {1}")]
    TooShort(usize, usize),
    #[error("Tried to modify an fd we weren't holding, {0}")]
    CloseFD(usize),
    #[error("Overwrote an FD slot, {0}")]
    OverwriteFD(usize),
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
            open_fds: vec![
                (0, FileInfo::STDIN),
                (1, FileInfo::STDOUT),
                (2, FileInfo::STDERR),
            ]
            .into_iter()
            .collect(),
        }
    }

    fn process(&mut self, ev: StraceEvent<'a>) -> Result<(), ProcessError> {
        self.syscalls.insert(ev.syscall);

        match ev.syscall {
            "openat" => {
                ProcessError::check_len(3, &ev.args)?;
                let _relflag = ev.args[0];
                let file = ev.args[1];
                let flags = ev.args[2];

                let fd = ev.result;

                if !fd.starts_with("-1") {
                    self.new_fd(fd.parse()?, FileInfo::new(file, flags))?
                }
            }
            "open" => {}
            "close" => {
                ProcessError::check_len(1, &ev.args)?;
                let fd = ev.args[0].parse()?;
                let result: i64 = ev.result.parse()?;
                if result == 0 {
                    self.close_fd(fd)?;
                }
            }
            _ => {}
        };

        Ok(())
    }

    fn new_fd(&mut self, id: usize, name: FileInfo<'a>) -> Result<(), ProcessError> {
        println!("Opening new fd {}: {:?}", id, name);
        match self.open_fds.insert(id, name) {
            Some(_) => Err(ProcessError::OverwriteFD(id)),
            None => Ok(()),
        }
    }

    fn close_fd(&mut self, id: usize) -> Result<(), ProcessError> {
        match self.open_fds.remove(&id) {
            Some(file) => {
                println!("Closing fd {}: {}", id, file.name);
                Ok(())
            }
            None => Err(ProcessError::CloseFD(id)),
        }
    }
}

fn main() {
    let mut args = std::env::args();
    if args.len() < 1 {
        println!("Not enough args. usage `cargo run [file]`");
    }
    let file = args.nth(1).unwrap();

    let file = std::fs::read_to_string(file).expect("couldn't open strace file");
    let re = Regex::new(r#"(\w+)\((.*)\)\s+= (.+)"#).unwrap();
    let args_re = Regex::new(r#"(.*),?(.+)?"#).unwrap();

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
        let e = parse_into_event(&re, &args_re, line).unwrap();

        match eventer.process(e) {
            Ok(_) => {}
            Err(err) => {
                println!("got eventer error on line {}: {}", lnnum, err);
            }
        }
    }
}

// strace -f -e trace=!write cargo build
// strace -f -s 1 cargo build
// strace -s 1 cargo build
//  strace -ff -o ./strace_all cargo build && cat strace_all.* > ./strace_all && rm strace_all.*
// rm -rf trace && mkdir trace && strace -ff -o ./trace/strace_all cargo build
