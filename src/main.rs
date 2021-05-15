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
    openfds: HashMap<usize, FileInfo<'a>>,
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
}

impl<'a> Eventer<'a> {
    fn new() -> Self {
        Self {
            syscalls: Default::default(),
            openfds: vec![
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
                println!("{:?} {}", ev.args, ev.result);
                if ev.args.len() < 3 {
                    return Err(ProcessError::TooShort(3, ev.args.len())); //invalid args
                }
                let file = ev.args[1];
                let flags = ev.args[2];

                self.new_fd(ev.result.parse()?, FileInfo::new(file, flags))
            }
            "open" => {}
            "close" => {}
            _ => {}
        }

        Ok(())
    }

    fn new_fd(&mut self, id: usize, name: FileInfo<'a>) {
        println!("Opening new fd {}: {:?}", id, name);
        self.openfds.insert(id, name);
    }
}

fn main() {
    let file = std::fs::read_to_string("./strace_all").unwrap();
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

        eventer.process(e);
    }
}

// strace -f -e trace=!write cargo build
// strace -f -s 1 cargo build
// strace -s 1 cargo build
//  strace -ff -o ./strace_all cargo build && cat strace_all.* > ./strace_all && rm strace_all.*
