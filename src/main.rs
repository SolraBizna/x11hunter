use std::{borrow::Cow, collections::HashMap, fs::File, io::Read, path::PathBuf};

use clap::Parser;
use nix::sys::stat::stat;
use nix::unistd::Uid;
use rand::{prelude::*, rng};

/// Snoop through your /proc and try to find out how to contact your desktop
/// session's X11 server. Outputs the results in a form ready to ingest into
/// your friendly neighborhood Bourne-compatible shell.
///
/// Best usage if you trust this program:
///
///     export `x11hunter`
///
/// Or, if you just want to run another program, this might work:
///
///     env `x11hunter` name_of_program args...
#[derive(Parser)]
struct Invocation {
    /// Only look for processes using a specific display, e.g. ":0"
    #[clap(short, long)]
    display: Option<String>,
    /// Add extra environment variables to force X11 over Wayland. This should
    /// only be necessary if you're running a Wayland session on your desktop,
    /// and the popular X server would *not* be its Xwayland instance.
    #[clap(short, long)]
    kill_wayland: bool,
    /// Be chatty.
    #[clap(short, long)]
    verbose: bool,
    /// Minimum number of processes to look at.
    #[clap(long, default_value = "10")]
    min: usize,
    /// Maximum number of processes to look at.
    #[clap(long, default_value = "50")]
    max: usize,
    /// Ideal percentage of processes to look at.
    #[clap(short, long, default_value = "25")]
    percent: usize,
    /// If, for some reason, you want to override which /proc directory you
    /// look inside, you can pass a path here.
    #[clap(long, default_value = "/proc")]
    proc_path: PathBuf,
}

// I write this function a lot, don't I?
/// Given a string, return a version of that string that is safe to paste
/// directly into a Bourne shell command line.
fn escape_for_shell(input: &'_ str) -> Cow<'_, str> {
    if input.chars().all(|ch| {
        ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' || ch == ':' || ch == '/'
    }) {
        return Cow::Borrowed(input);
    }
    let mut ret =
        String::with_capacity(input.len() + 2 + input.chars().filter(|x| *x == '\'').count() * 4);
    ret.push('\'');
    for ch in input.chars() {
        if ch == '\'' {
            ret += r#"'\''"#;
        } else {
            ret.push(ch);
        }
    }
    ret.push('\'');
    Cow::Owned(ret)
}

struct EnvList {
    envs: Vec<String>,
}

impl EnvList {
    pub fn new() -> EnvList {
        EnvList { envs: vec![] }
    }
    pub fn add(&mut self, name: &str, value: &str) {
        self.envs.push(format!(
            "{}={}",
            escape_for_shell(name),
            escape_for_shell(value)
        ));
    }
    pub fn print_out(&self) {
        for (i, env) in self.envs.iter().enumerate() {
            if i != 0 {
                print!(" ");
            }
            print!("{env}");
        }
        println!();
    }
}

/// I don't have anything against Wayland, but if you're trying to get
/// something to run on an X server on a system that a Wayland server is
/// running on, you need these...
fn kill_wayland(env_list: &mut EnvList) {
    env_list.add("GDK_BACKEND", "x11");
    env_list.add("QT_QPA_PLATFORM", "xcb");
    env_list.add("CLUTTER_BACKEND", "x11");
    env_list.add("SDL_VIDEO_DRIVER", "x11");
    env_list.add("SDL_VIDEODRIVER", "x11");
    env_list.add("XDG_SESSION_TYPE", "x11");
    env_list.add("ELM_DISPLAY", "x11");
    env_list.add("WINIT_UNIX_BACKEND", "x11"); // Obsolete but observed
    //env_list.add("ELECTRON_OZONE_PLATFORM_HINT", "x11"); // Obsolete
}

fn main() {
    let invocation = Invocation::parse();
    if invocation.max < invocation.min {
        eprintln!("--max must be greater than or equal to --min");
        std::process::exit(1);
    }
    if invocation.percent > 100 {
        eprintln!("--percent must be in the range 0 to 100 inclusive");
        std::process::exit(1);
    }
    let mut env_list = EnvList::new();
    // Step 1: See what we can see in `/proc`.
    let me = Uid::effective(); // should this be real() instead?
    let mut procs = vec![];
    for ent in std::fs::read_dir(&invocation.proc_path).expect("unable to walk proc") {
        let ent = ent.expect("error while walking proc");
        let path = ent.path();
        let file_name = ent.file_name();
        let Some(file_name) = file_name.to_str() else {
            if invocation.verbose {
                eprintln!("Skipping {:?} (non-Unicode chars in filename)", path);
            }
            continue;
        };
        if !file_name.chars().all(|x| x.is_ascii_digit()) {
            if invocation.verbose {
                eprintln!("Skipping {:?} (non-digit chars in filename)", path);
            }
            continue;
        };
        let stat = stat(&path).expect("error while looking at a subdirectory of proc");
        let dir_owner = Uid::from_raw(stat.st_uid);
        if me != dir_owner {
            if invocation.verbose {
                eprintln!("Skipping {:?} (not my process)", path);
            }
            continue;
        }
        procs.push(path);
    }
    if procs.is_empty() {
        eprintln!("Didn't find any processes owned by this user. Nothing we can do.");
        std::process::exit(1);
    }
    if invocation.verbose {
        eprintln!(
            "Found {} proc{}. Shuffling.",
            procs.len(),
            if procs.len() == 1 { "" } else { "s" }
        );
    }
    let mut rng = rng();
    procs.shuffle(&mut rng);
    let look_at = ((procs.len() * invocation.percent + 50) / 100)
        .min(invocation.max)
        .max(invocation.min);
    let mut pop: HashMap<(String, Option<String>), usize> = HashMap::new();
    let mut looked_at = 0;
    let mut env_buf = String::new();
    let mut zero_index_buf = vec![];
    for path in procs.into_iter() {
        if looked_at >= look_at {
            if invocation.verbose {
                eprintln!("We've seen enough. Let's decide.");
            }
            break;
        }
        let Ok(mut file) = File::open(path.join("environ")) else {
            if invocation.verbose {
                eprintln!("Skipping {:?} (vanished or inaccessible)", path);
            }
            continue;
        };
        env_buf.clear();
        file.read_to_string(&mut env_buf).unwrap();
        zero_index_buf.clear();
        zero_index_buf.extend(env_buf.match_indices('\0').map(|(a, _)| a));
        let mut display_value = None;
        let mut xauthority_value = None;
        for (start, end) in std::iter::once(0)
            .chain(zero_index_buf.iter().copied().map(|x| x + 1))
            .zip(zero_index_buf.iter().copied())
        {
            let kv = &env_buf[start..end];
            let Some((k, v)) = kv.split_once("=") else {
                continue;
            };
            if k.is_empty() || v.is_empty() {
                continue;
            }
            if k == "DISPLAY" {
                if let Some(whitelist_display) = invocation.display.as_ref()
                    && v != whitelist_display
                {
                    if invocation.verbose {
                        eprintln!("Skipping {:?} (wrong DISPLAY)", path);
                    }
                    continue;
                }
                display_value = Some(v);
            } else if k == "XAUTHORITY" {
                xauthority_value = Some(v);
            }
            if display_value.is_some() && xauthority_value.is_some() {
                break;
            }
        }
        if let Some(display_value) = display_value {
            if invocation.verbose {
                eprintln!("{path:?}: DISPLAY={display_value:?} XAUTHORITY={xauthority_value:?}");
            }
            looked_at += 1;
            *pop.entry((
                display_value.to_string(),
                xauthority_value.map(str::to_string),
            ))
            .or_default() += 1;
        } else {
            if invocation.verbose {
                eprintln!("Skipping {:?} (no DISPLAY)", path);
            }
            continue;
        }
        drop(file);
    }
    if looked_at == 0 {
        eprintln!("Didn't find DISPLAY in any processes.");
        std::process::exit(1);
    }
    let mut pop: Vec<((String, Option<String>), usize)> = pop.into_iter().collect();
    pop.sort_by_key(|(_, x)| !x); // sort in descending order of pop
    if invocation.verbose {
        eprintln!("Results of the popularity contest:");
        for (i, ((display, xauthority), pop)) in pop.iter().enumerate() {
            eprintln!(
                "    #{n}: DISPLAY={display:?} XAUTHORITY={xauthority:?} population={pop}",
                n = i + 1,
            );
        }
    }
    let ((display, xauthority), _) = pop.into_iter().next().unwrap();
    env_list.add("DISPLAY", &display);
    if let Some(xauthority) = xauthority {
        env_list.add("XAUTHORITY", &xauthority);
    }
    if invocation.kill_wayland {
        kill_wayland(&mut env_list);
    }
    env_list.print_out();
}
