use rgtk::*;
use rustc_serialize::{Encodable, json};
use std::cell::Cell;
use std::env;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::fs::{self, PathExt};
use std::ops::Deref;
use std::path::{Path, PathBuf};

pub static WINDOW_WIDTH : i32 = 1242;
pub static WINDOW_HEIGHT : i32 = 768;
pub static EDITOR_HEIGHT_PCT : f32 = 0.70;
pub static MIN_FONT_SIZE : i32 = 0;
pub static MAX_FONT_SIZE : i32 = 50;

pub static DATA_DIR : &'static str = ".soak";
pub static CONFIG_FILE : &'static str = ".soakrc";
pub static CONFIG_CONTENT : &'static str = include_str!("../resources/soakrc");
pub static PREFS_FILE : &'static str = "prefs.json";
pub static NO_WINDOW_FLAG : &'static str = "-nw";

pub struct Resource {
    pub path: &'static [&'static str],
    pub data: &'static str,
    pub always_copy: bool
}
pub static DATA_CONTENT : &'static [Resource] = &[
    Resource{path: &["after", "syntax", "rust.vim"],
             data: include_str!("../resources/soak/after/syntax/rust.vim"),
             always_copy: true},

    Resource{path: &["autoload", "paste.vim"],
             data: include_str!("../resources/soak/autoload/paste.vim"),
             always_copy: false},
    Resource{path: &["autoload", "rust.vim"],
             data: include_str!("../resources/soak/autoload/rust.vim"),
             always_copy: true},

    Resource{path: &["compiler", "rustc.vim"],
             data: include_str!("../resources/soak/compiler/rustc.vim"),
             always_copy: true},
    Resource{path: &["compiler", "cargo.vim"],
             data: include_str!("../resources/soak/compiler/cargo.vim"),
             always_copy: true},

    Resource{path: &["doc", "rust.txt"],
             data: include_str!("../resources/soak/doc/rust.txt"),
             always_copy: true},

    Resource{path: &["ftdetect", "rust.vim"],
             data: include_str!("../resources/soak/ftdetect/rust.vim"),
             always_copy: true},

    Resource{path: &["ftplugin", "rust.vim"],
             data: include_str!("../resources/soak/ftplugin/rust.vim"),
             always_copy: true},
    Resource{path: &["ftplugin", "c.vim"],
             data: include_str!("../resources/soak/ftplugin/c.vim"),
             always_copy: false},

    Resource{path: &["indent", "rust.vim"],
             data: include_str!("../resources/soak/indent/rust.vim"),
             always_copy: true},
    Resource{path: &["indent", "c.vim"],
             data: include_str!("../resources/soak/indent/c.vim"),
             always_copy: false},

    Resource{path: &["plugin", "eunuch.vim"],
             data: include_str!("../resources/soak/plugin/eunuch.vim"),
             always_copy: false},
    Resource{path: &["plugin", "racer.vim"],
             data: include_str!("../resources/soak/plugin/racer.vim"),
             always_copy: true},
    Resource{path: &["plugin", "rust.vim"],
             data: include_str!("../resources/soak/plugin/rust.vim"),
             always_copy: true},

    Resource{path: &["syntax", "c.vim"],
             data: include_str!("../resources/soak/syntax/c.vim"),
             always_copy: false},
    Resource{path: &["syntax", "nosyntax.vim"],
             data: include_str!("../resources/soak/syntax/nosyntax.vim"),
             always_copy: false},
    Resource{path: &["syntax", "rust.vim"],
             data: include_str!("../resources/soak/syntax/rust.vim"),
             always_copy: true},
    Resource{path: &["syntax", "syncolor.vim"],
             data: include_str!("../resources/soak/syntax/syncolor.vim"),
             always_copy: false},
    Resource{path: &["syntax", "synload.vim"],
             data: include_str!("../resources/soak/syntax/synload.vim"),
             always_copy: false},
    Resource{path: &["syntax", "syntax.vim"],
             data: include_str!("../resources/soak/syntax/syntax.vim"),
             always_copy: false},

    Resource{path: &["syntax_checkers", "rust", "rustc.vim"],
             data: include_str!("../resources/soak/syntax_checkers/rust/rustc.vim"),
             always_copy: true},

    Resource{path: &["filetype.vim"],
             data: include_str!("../resources/soak/filetype.vim"),
             always_copy: false}
];

pub struct State<'a> {
    pub projects: HashSet<String>,
    pub expansions: HashSet<String>,
    pub selection: Option<String>,
    pub easy_mode: bool,
    pub font_size: i32,
    pub builders: HashMap<PathBuf, (gtk::VteTerminal, Cell<i32>)>,
    pub window: &'a gtk::Window,
    pub tree_store: &'a gtk::TreeStore,
    pub tree_model: &'a gtk::TreeModel,
    pub tree_selection: &'a gtk::TreeSelection,
    pub rename_button: &'a gtk::Button,
    pub remove_button: &'a gtk::Button,
    pub is_refreshing_tree: bool
}

#[derive(RustcDecodable, RustcEncodable)]
struct Prefs {
    projects: Vec<String>,
    expansions: Vec<String>,
    selection: Option<String>,
    easy_mode: bool,
    font_size: i32
}

pub fn get_home_dir() -> PathBuf {
    if let Some(path) = env::home_dir() {
        path
    } else {
        PathBuf::from(".")
    }
}

fn get_prefs(state: &State) -> Prefs {
    Prefs {
        projects: state.projects.clone().into_iter().collect(),
        expansions: state.expansions.clone().into_iter().collect(),
        selection: state.selection.clone(),
        easy_mode: state.easy_mode,
        font_size: state.font_size
    }
}

pub fn is_parent_path(parent_str: &String, child_str: &String) -> bool {
    let parent_ref: &str = parent_str.as_ref();
    child_str.starts_with(parent_ref) &&
    Path::new(parent_str).parent() != Path::new(child_str).parent()
}

pub fn get_selected_path(state: &State) -> Option<String> {
    let mut iter = gtk::TreeIter::new().unwrap();

    if state.tree_selection.get_selected(state.tree_model, &mut iter) {
        state.tree_model.get_value(&iter, 1).get_string()
    } else {
        None
    }
}

fn is_project_path(path: &Path) -> bool {
    path.join("Cargo.toml").exists()
}

fn is_project_root(state: &State, path: &Path) -> bool {
    if let Some(path_str) = path.to_str() {
        state.projects.contains(&path_str.to_string())
    } else {
        false
    }
}

pub fn get_project_path(state: &State, path: &Path) -> Option<PathBuf> {
    if is_project_path(path) || is_project_root(state, path) {
        Some(PathBuf::from(path))
    } else {
        if let Some(parent_path) = path.parent() {
            get_project_path(state, parent_path.deref())
        } else {
            None
        }
    }
}

pub fn get_selected_project_path(state: &State) -> Option<PathBuf> {
    if let Some(path_str) = get_selected_path(state) {
        get_project_path(state, Path::new(&path_str))
    } else {
        None
    }
}

pub fn write_prefs(state: &State) {
    let prefs = get_prefs(state);

    let mut json_str = String::new();
    {
        let mut encoder = json::Encoder::new_pretty(&mut json_str);
        prefs.encode(&mut encoder).ok().expect("Error encoding prefs.");
    }

    let prefs_path = get_home_dir().deref().join(DATA_DIR).join(PREFS_FILE);
    if let Some(mut f) = fs::File::create(&prefs_path).ok() {
        match f.write(json_str.as_bytes()) {
            Ok(_) => {},
            Err(e) => println!("Error writing prefs: {}", e)
        };
    }
}

pub fn read_prefs(state: &mut State) {
    let prefs_path = get_home_dir().deref().join(DATA_DIR).join(PREFS_FILE);
    if let Some(mut f) = fs::File::open(&prefs_path).ok() {
        let mut json_str = String::new();
        let prefs_option : Option<Prefs> = match f.read_to_string(&mut json_str) {
            Ok(_) => {
                match json::decode(json_str.as_ref()) {
                    Ok(object) => Some(object),
                    Err(e) => {
                        println!("Error decoding prefs: {}", e);
                        None
                    }
                }
            },
            Err(_) => None
        };

        if let Some(prefs) = prefs_option {
            state.projects.clear();
            for path_str in prefs.projects.iter() {
                state.projects.insert(path_str.clone());
            }

            state.expansions.clear();
            for path_str in prefs.expansions.iter() {
                state.expansions.insert(path_str.clone());
            }

            state.selection = prefs.selection;
            state.easy_mode = prefs.easy_mode;

            if (prefs.font_size >= MIN_FONT_SIZE) && (prefs.font_size <= MAX_FONT_SIZE) {
                state.font_size = prefs.font_size;
            }
        }
    }
}
