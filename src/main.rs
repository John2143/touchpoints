use nom::{
    bytes::complete::{tag, take_until},
    IResult,
};

#[derive(Debug)]
struct StraceEvent<'a> {
    syscall: &'a str,
    args: &'a str,
    result: &'a str,
}

fn parse_into_event(input: &str) -> IResult<&str, StraceEvent> {
    let (input, syscall) = take_until("(")(input)?;

    Ok((
        input,
        StraceEvent {
            syscall,
            args: "",
            result: "",
        },
    ))
}

fn main() {
    let file = std::fs::read_to_string("./strace_all").unwrap();

    for line in file.lines() {
        if line.starts_with("+++") {
            continue;
        }
        if line.starts_with("---") {
            continue;
        }

        let e = parse_into_event(line).unwrap();

        dbg!(&e);
    }
}

// strace -f -e trace=!write cargo build
// strace -f -s 1 cargo build
// strace -s 1 cargo build
//  strace -ff -o ./strace_all cargo build && cat strace_all.* > ./strace_all && rm strace_all.*
