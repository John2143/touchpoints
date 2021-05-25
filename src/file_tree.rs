use std::{
    ffi::{OsStr, OsString},
    str::FromStr,
};

//use bumpalo::{boxed::Box as BBox, Bump};

#[derive(Debug, PartialEq, Eq)]
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
            Self::Dir(ref mut n) => n,
            Self::File(_) => panic!("inode which was previously a directory is now a file"),
        }
    }

    fn print(&self, indent: usize) {
        match self {
            Self::File(perms) => {
                //println!("{}{:?}", spaces(indent), perms);
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
            match value {
                Node::Dir(d) => {
                    println!(
                        "{}{} {:?} {}",
                        spaces(indent),
                        key.to_string_lossy(),
                        d.stain,
                        d.contained_files,
                    );
                    value.print(indent + 1)
                }
                Node::File(perms) => {
                    println!("{}{} {:?}", spaces(indent), key.to_string_lossy(), perms);
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

            //Ideally, we'd delay allocation of the key until it is needed.
            //This requires the raw_entry api.
            //
            //#![feature(hash_raw_entry)]
            //
            // > Raw entries are useful for such exotic situations as:
            // > - ...
            // > - Deferring the creation of an owned key until it is known to be required
            // > - ...
            //
            // ideal(ish) code below:
            //
            // let empty = || (p.to_os_string(), Node::Dir(Box::new(Default::default())));
            // let (_, val) = cwd.items.raw_entry_mut().from_key(*p).or_insert_with(empty);
            // cwd = val.dir();
            cwd = cwd
                .items
                .entry(folder.to_os_string())
                .or_insert(Node::Dir(Box::new(Default::default())))
                .dir();
        }

        //Finally, insert file
        cwd.contained_files += 1;
        cwd.items.insert(filename.to_os_string(), Node::File(flags));
    }
}
