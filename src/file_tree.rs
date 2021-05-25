use std::{
    ffi::{OsStr, OsString},
    str::FromStr,
};

use hashbrown::HashMap;
use tracing::{info, trace};

//use bumpalo::{boxed::Box as BBox, Bump};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Perms {
    Read,
    Write,
}

impl Default for Perms {
    fn default() -> Self {
        Self::Read
    }
}

#[derive(Debug)]
pub enum Node {
    Dir(Box<Directory>),
    File(Perms),
}

#[derive(Default, Debug)]
pub struct Directory {
    items: HashMap<OsString, Node>,
    stain: Perms,
    contained_files: usize,
}

#[derive(Default, Debug)]
pub struct FileTree {
    root: Directory,
}

impl Node {
    fn dir(&mut self) -> &mut Directory {
        match self {
            //expected path: we are already a dir
            Self::Dir(ref mut n) => n,
            Self::File(f) => {
                let f = f.clone();
                info!("inode which was previously a directory is now a file");
                let mut new_dir = Directory::default();
                new_dir.items.insert(OsString::from("."), Node::File(f));
                *self = Self::Dir(Box::new(new_dir));
                self.dir()
            }
        }
    }

    fn empty_dir() -> Self {
        Self::Dir(Default::default())
    }

    fn print(&self, indent: usize) {
        match self {
            Self::File(_) => {}
            Self::Dir(dir) => dir.print(indent + 1),
        }
    }
}

fn spaces(n: usize) -> &'static str {
    let spaces = "                                               ";
    &spaces[..n]
}

impl Perms {
    fn color(&self) -> ansi_term::Colour {
        match self {
            Self::Write => ansi_term::Colour::Red,
            Self::Read => ansi_term::Color::Blue,
        }
    }
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
            let key_string = key.to_string_lossy();
            match value {
                Node::Dir(d) => {
                    info!("{} {:?} {}", key_string, d.stain, d.contained_files);
                    println!("{}{}", spaces(indent), d.stain.color().paint(key_string),);

                    value.print(indent + 1)
                }
                Node::File(perms) => {
                    info!("{:?} {:?}", key_string, perms);
                    println!("{}{}", spaces(indent), perms.color().paint(key_string));
                }
            }
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

        let flags: Perms = flags.parse().unwrap();

        let mut cwd = &mut self.root;

        use itertools::Itertools;
        let mut filename = OsStr::new(".");

        //We now need to walk the directory structure incrementing the file count each time we pass
        //a directory. If we are writing to the file, stain the directory with a write marker.
        //for example: `/home/me/bin/rustc` gets split into `/, home, me, bin, rustc`, then iterated in pairs
        // (home, me)
        // (me, bin)
        // (bin, rustc)
        // Finally, at the end, filename is rustc and we have the cwd of `/home/me/bin/`
        for (folder, next) in path_buf.iter().skip(1).tuple_windows() {
            filename = next;
            if flags == Perms::Write {
                cwd.stain = Perms::Write;
            }
            cwd.contained_files += 1;

            let empty = || (folder.to_os_string(), Node::empty_dir());

            let (key, val) = cwd
                .items
                .raw_entry_mut()
                .from_key(folder)
                .or_insert_with(empty);


            trace!("key is {:?} + {:?}", key, next);

            cwd = val.dir();

            //match val {
            ////expected path: we inserted a dir or we are partway done parsing
            //Node::Dir(d) => {
            //cwd = &mut *d;
            //continue;
            //}
            ////rare path: caused when stating directories or something
            //Node::File(f) => f,
            //};
        }

        //Finally, insert file
        cwd.contained_files += 1;
        cwd.items.insert(filename.to_os_string(), Node::File(flags));
    }
}
