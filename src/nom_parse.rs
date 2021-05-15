use nom::{
    branch::alt,
    bytes::complete::{escaped, is_not, tag, take_till, take_till1, take_until},
    character::complete::one_of,
    IResult,
};

use crate::StraceEvent;

pub fn read_while_args(mut input: &str) -> IResult<&str, Vec<&str>> {
    //TODO this sucks
    let mut args = vec![];

    input = &input[1..];

    loop {
        let (rest, arg) = take_till(|c| c == ',' || c == '"' || c == ')')(input)?;
        input = rest;

        match input.chars().next().unwrap() {
            ',' => {
                args.push(arg);
            }
            '"' => {
                input = &input[1..]; // skip quote
                if input.chars().next().unwrap() == '"' {
                    args.push("");
                    input = &input[1..];
                } else {
                    let (rest, arg) = escaped(is_not("\""), '\\', one_of(r#""'n"#))(input)?;
                    args.push(arg);
                    dbg!(arg);
                    input = &rest[1..];
                }
            }
            ')' => {
                args.push(arg);
                return Ok((&input[1..], args));
            }
            _ => unreachable!(),
        };

        input = &input[2..];
    }
}

pub fn parse_into_event(input: &str) -> IResult<&str, StraceEvent> {
    let (input, syscall) = take_until("(")(input)?;
    let (input, args) = read_while_args(input)?;
    let result = input;
    let input = "";

    Ok((
        input,
        StraceEvent {
            syscall,
            args,
            result,
        },
    ))
}
