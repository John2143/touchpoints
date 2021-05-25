use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    str::FromStr,
};

//use bumpalo::{boxed::Box as BBox, Bump};

#[derive(Debug)]
pub enum Perms {
    Read,
    Write,
}

#[derive(Debug)]
pub enum Node {
    Dir(Box<Directory>),
    File(Perms),
}

#[derive(Default, Debug)]
pub struct Directory {
    items: HashMap<OsString, Node>,
}

#[derive(Default, Debug)]
pub struct FileTree {
    root: Directory,
}

impl Node {
    fn dir(&mut self) -> &mut Directory {
        match self {
            Self::Dir(ref mut n) => n,
            Self::File(_) => panic!("inode which was previously a directory is now a file"),
        }
    }

    fn print(&self, indent: usize) {
        match self {
            Self::File(perms) => {
                println!("{}{:?}", spaces(indent), perms);
            }
            Self::Dir(dir) => dir.print(indent + 1),
        }
    }
}

fn spaces(n: usize) -> &'static str {
    let spaces = "                                               ";
    &spaces[..n]
}

impl FromStr for Perms {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        for perm in s.split("|") {
            match perm {
                "O_WRONLY" | "O_RDWR" => return Ok(Self::Write),
                _ => {}
            };
        }
        Ok(Self::Read)
    }
}

impl Directory {
    fn print(&self, indent: usize) {
        for (key, value) in self.items.iter() {
            println!("{}{:?}", spaces(indent), key);
            value.print(indent + 1)
        }
    }
}

impl FileTree {
    pub fn new<'a>(items: impl IntoIterator<Item = crate::FDInfo<'a>>) -> Self {
        let mut tree = Self::default();

        for i in items {
            tree.insert(i);
        }

        tree
    }

    pub fn print(&self) {
        self.root.print(0);
    }

    fn insert<'a>(&mut self, file: crate::FDInfo<'a>) {
        let (path_buf, flags) = match file {
            crate::FDInfo::File { path_buf, flags } => (path_buf, flags),
            _ => return,
        };

        let iter: Vec<_> = path_buf.iter().skip(1).collect();
        let wd = OsStr::new(".");

        let (filename, iter) = if path_buf.is_dir() {
            (&wd, iter.as_slice())
        } else {
            iter.split_last()
                .expect("path did not have at least a filename")
        };


        let mut cwd = &mut self.root;

        for p in iter {
            cwd = cwd
                .items
                .entry(p.to_os_string())
                .or_insert(Node::Dir(Box::new(Default::default())))
                .dir();
        }

        cwd.items
            .insert(filename.to_os_string(), Node::File(flags.parse().unwrap()));
    }
}
