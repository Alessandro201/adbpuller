use anyhow::{anyhow, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashSet;
use std::fs::{read_to_string, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::time::Duration;

use std::{env, process};
use unix_path::{Path as UnixPath, PathBuf as UnixPathBuf};

use which::which;

use clap::{ArgAction, Args, Parser};
use colored::Colorize;

use normpath::BasePathBuf;
use normpath::PathExt;

#[derive(Args, Debug)]
#[group(required = true, multiple = true)]
struct Sources {
    /// The folder(s) or item(s) to copy
    #[arg(short, long, num_args = 0..,)]
    sources: Vec<UnixPathBuf>,

    /// Add /sdcard/DCIM and /sdcard/Pictures to the sources
    #[arg(short = 'm', long = "copy-media")]
    media_preset: bool,

    /// Add Whatsapp Audio, Images, Video and Voice Notes to the sources
    #[arg(short = 'w', long = "copy-whatsapp")]
    whatsapp_preset: bool,

    /// Add Whatsapp Backup and Databases folders to the sources
    #[arg(short = 'b', long = "copy-whatsapp-backups")]
    whatsapp_backups_preset: bool,
}

/// Pull files from android using ADB drivers
#[derive(Parser, Debug)]
#[command(version, about)]
#[command(long_about = "Pull files from android using ADB drivers

Example:
    ./adb_puller.exe -s /sdcard/DCIM")]
struct Cli {
    #[command(flatten)]
    source: Sources,

    /// The folder in which to copy the files
    #[arg(short, long, default_value = ".")]
    dest: PathBuf,

    /// Skip files written in a file
    #[arg(long, value_parser, num_args = 0..)]
    skip: Option<Vec<PathBuf>>,

    /// Print which files would be copied and where
    #[arg(long, action = ArgAction::SetTrue)]
    dry_run: bool,

    /// Overwrite files already present in the destination folder.
    #[arg(short, long = "force", action = ArgAction::SetTrue)]
    force: bool,

    /// Don't copy metadata such as last modification date ecc..
    #[arg(long = "no-metadata", action = ArgAction::SetTrue)]
    no_metadata: bool,
}

impl Cli {
    fn check_sources(&mut self) {
        let mut sources: Vec<UnixPathBuf> = Vec::new();

        if self.source.media_preset {
            sources.extend([UnixPathBuf::from("/sdcard/DCIM"), UnixPathBuf::from("/sdcard/Pictures")])
        }

        if self.source.whatsapp_preset {
            sources.extend([
                UnixPathBuf::from("/sdcard/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Audio"),
                UnixPathBuf::from("/sdcard/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Images"),
                UnixPathBuf::from("/sdcard/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Video"),
                UnixPathBuf::from("/sdcard/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Voice Notes"),
                UnixPathBuf::from("/sdcard/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Video Notes"),
                UnixPathBuf::from("/sdcard/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Documents"),
            ])
        }

        if self.source.whatsapp_backups_preset {
            sources.extend([
                UnixPathBuf::from("/sdcard/Android/media/com.whatsapp/WhatsApp/Backups"),
                UnixPathBuf::from("/sdcard/Android/media/com.whatsapp/WhatsApp/Databases"),
            ])
        }

        self.source.sources.extend(sources);
    }
}

fn get_files_from_adb(adb_path: &PathBuf, root_path: &UnixPathBuf) -> Vec<UnixPathBuf> {
    let mut file_list: Vec<UnixPathBuf> = Vec::new();

    let mut cmd = process::Command::new(adb_path);
    cmd.arg("shell");
    cmd.arg("ls");
    cmd.arg("-R");
    cmd.arg(format!("\"{}\"", root_path.as_unix_str().to_str().unwrap()));

    // println!("Running {:#?}", cmd);

    let output = cmd.output().expect("Failed to execute the command").stdout;

    let mut lines: Vec<String> = Vec::new();
    match String::from_utf8(output) {
        Ok(s) => lines.extend(s.lines().map(|x| x.trim().to_string())),
        Err(err) => {
            println!("Unable to read the output of `adb shell ls -R <path>`: {:#?}", err);
            return file_list;
        }
    }

    lines.retain(|x| !x.is_empty());

    if lines.len() == 1 {
        file_list.push(UnixPathBuf::from(&lines[0]))
    }

    let mut current_folder_root: UnixPathBuf = UnixPathBuf::from(&root_path); // default, but should be changed right away
    for line in lines.into_iter() {
        if line.starts_with('/') {
            current_folder_root = UnixPathBuf::from(&line[..line.len() - 1]);
            if let Some(i) = file_list.iter().position(|x| x == &current_folder_root) {
                file_list.remove(i);
            }
        } else {
            let file_path = current_folder_root.join(line);
            file_list.push(file_path);
        }
    }

    file_list
}

struct SrcDestFiles {
    src_files: Vec<UnixPathBuf>,
    dest_files: Vec<BasePathBuf>,
}

impl SrcDestFiles {
    fn new() -> Self {
        Self {
            src_files: vec![],
            dest_files: vec![],
        }
    }

    /// Moves all the elements of `other` into `self`, leaving `other` empty.
    fn append(&mut self, other: &mut SrcDestFiles) {
        self.src_files.append(&mut other.src_files);
        self.dest_files.append(&mut other.dest_files);
    }

    fn is_empty(&self) -> bool {
        self.src_files.is_empty()
    }

    fn len(&self) -> usize {
        self.src_files.len()
    }
}

impl IntoIterator for SrcDestFiles {
    type Item = (UnixPathBuf, BasePathBuf);
    type IntoIter = SrcDestFilesIntoIterator;

    fn into_iter(self) -> Self::IntoIter {
        SrcDestFilesIntoIterator { files: self, index: 0 }
    }
}

struct SrcDestFilesIntoIterator {
    files: SrcDestFiles,
    index: usize,
}

impl Iterator for SrcDestFilesIntoIterator {
    type Item = (UnixPathBuf, BasePathBuf);

    fn next(&mut self) -> Option<(UnixPathBuf, BasePathBuf)> {
        let src_path = match self.files.src_files.get(self.index) {
            Some(path) => path.clone(),
            None => return None,
        };
        let dest_path = match self.files.dest_files.get(self.index) {
            Some(path) => path.clone(),
            None => return None,
        };
        self.index += 1;
        Some((src_path, dest_path))
    }
}

fn get_files_to_skip(skip: &Option<Vec<PathBuf>>) -> HashSet<String> {
    let mut hs: HashSet<String> = HashSet::new();
    if let Some(skip_inside) = skip {
        for path in skip_inside {
            for line in read_to_string(path).unwrap_or_default().lines().map(String::from) {
                hs.insert(line);
            }
        }
    }
    hs
}

fn connected_to_adb_server(adb_path: &PathBuf, retries: Option<usize>) -> bool {
    let retries = retries.unwrap_or(1);

    let output = match process::Command::new(adb_path).arg("devices").stdout(process::Stdio::piped()).output() {
        Ok(output) => output,
        Err(_) => {
            println!(
                "Unable to check if adb is connected. \nADB path: \"{}\"",
                adb_path.as_path().to_str().unwrap()
            );
            exit(1);
        }
    };

    let out_vec = output.stdout.to_vec();
    let out_string = String::from_utf8(out_vec).unwrap();

    // `adb devices` outputs the devices attached to the adb server after `List of devices attached`
    // If that line is the last line it means that no device is attached
    if !out_string.trim_end().ends_with("List of devices attached") {
        true
    } else if retries > 0 {
        connected_to_adb_server(adb_path, Some(retries - 1))
    } else {
        false
    }
}

fn get_adb_path() -> Result<PathBuf> {
    let adb_name = if cfg!(windows) {
        "adb.exe"
    } else if cfg!(unix) {
        "adb"
    } else {
        return Err(anyhow!("OS is not supported"));
    };

    let adb_path = env::current_exe()
        .context("Failed to get path of the adbpuller executable")?
        .parent()
        .context("Unable to get the parent folder of the adbpuller executable")?
        .join(adb_name);

    if adb_path.exists() {
        Ok(adb_path)
    } else {
        which("adb").context("Unable to find adb drivers. Download and add them to $PATH")
    }

    // adb_path.normalize().or_else(|_| {
    //     println!("Unable to find adb in the .");
    //
    //     if let Ok(path) = which("adb").expect("Unable to find adb.").normalize() {
    //         println!("Using adb from $PATH");
    //         Ok(path)
    //     } else {
    //         Err(anyhow!("adb is not installed in the system. Download it and add it to $PATH"))
    //     }
    // })
}

fn build_file_list(adb_path: &PathBuf, args: &Cli) -> SrcDestFiles {
    let files_to_skip = get_files_to_skip(&args.skip);
    let mut files = SrcDestFiles::new();

    for root_src in args.source.sources.iter() {
        let mut file_list = get_files_from_adb(adb_path, root_src);
        println!("{:7} files found in {:?}", file_list.len(), &root_src);
        file_list.retain(|x| !files_to_skip.contains(x.to_str().unwrap()));

        let mut temp_files = build_destination_files(&file_list, args.dest.as_path(), root_src, args.force);
        println!("{:7} to copy", temp_files.len());

        files.append(&mut temp_files)
    }
    files
}

fn build_destination_files(file_list: &[UnixPathBuf], root_dest: &Path, root_src: &UnixPathBuf, force: bool) -> SrcDestFiles {
    let mut files = SrcDestFiles::new();

    for file in file_list.iter() {
        let file_rel_to_src: &UnixPath = match file.strip_prefix(root_src.parent().unwrap()) {
            Ok(path) => path,
            Err(_) => {
                println!(
                    "Unable to strip the prefix {:?} from {:?} when tying to find its corresponding destination",
                    &root_src, &file
                );
                continue;
            }
        };

        let dest = root_dest.join(file_rel_to_src.as_unix_str().to_str().unwrap());

        // #[cfg(target_os = "windows")]
        // {
        //     let dest = match dest.normalize_virtually() {
        //         Ok(p) => p,
        //         Err(err) => {
        //             println!("Unable to normalize destination path: {:?} due to error: {err}", dest);
        //             continue;
        //         }
        //     };
        // }

        if dest.exists() && !force {
            continue;
        }

        files.src_files.push(file.to_owned());
        files.dest_files.push(BasePathBuf::new(dest).unwrap());
    }

    files
}

fn main() {
    let args: Cli = {
        // Limit scope to remove mutability
        let mut args = Cli::parse();
        args.check_sources();
        args
    };

    let adb_path = match get_adb_path() {
        Ok(path) => {
            println!("Using adb from: {path:?}");
            path
        }
        Err(err) => {
            eprintln!("{}", err);
            exit(1)
        }
    };

    println!("Checking if a device is attached to adb server..");
    if !connected_to_adb_server(&adb_path, None) {
        println!("No device found. Try executing \"{} devices\"", adb_path.as_path().to_str().unwrap());
        exit(1);
    }

    println!("Building file list, it may take some time...");

    let files = build_file_list(&adb_path, &args);

    if args.source.sources.len() > 1 {
        println!("\n{} total files to copy", files.dest_files.len());
    }

    // Print files to copy if --dry-run
    if args.dry_run && !files.is_empty() {
        let mut user_input = String::new();

        while user_input.trim().to_lowercase() != "y" && user_input.trim().to_lowercase() != "n" {
            print!("Do you want to print the files and their destinations? [y/N]: ");
            let _ = std::io::stdout().flush();
            user_input.clear();
            let _ = std::io::stdin().read_line(&mut user_input);
        }

        if user_input.trim().to_lowercase() == "y" {
            for (src_file, dest_file) in files.into_iter() {
                println!(
                    "{}  {}  {}",
                    src_file.to_str().unwrap().green(),
                    "->".cyan(),
                    dest_file.as_path().to_str().unwrap()
                );
            }
        }
        exit(0)
    }

    if files.is_empty() {
        println!("No files found to copy. Exiting..");
        exit(0)
    }

    let mut files_done: Vec<UnixPathBuf> = Vec::new();
    let mut files_failed: Vec<UnixPathBuf> = Vec::new();

    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{bar:.cyan/blue}] {human_pos:>7}/{human_len:7} ({eta}) {wide_msg}")
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.enable_steady_tick(Duration::from_millis(50));

    for (src_file, dest_file) in files.into_iter() {
        pb.set_message(format!("{}", src_file.display()));
        pb.inc(1);

        if let Err(err) = std::fs::create_dir_all(dest_file.parent().unwrap().unwrap().as_path()) {
            println!(
                "Error in creating directory: \"{}\". Skipping file: {} \nErr:{err}",
                dest_file.parent().unwrap().unwrap().as_path().display(),
                src_file.display(),
            );
            files_failed.push(src_file);
            continue;
        };

        let status = process::Command::new(&adb_path)
            .arg("pull")
            .arg("-a")
            .arg(src_file.as_path().as_unix_str().to_str().unwrap())
            .arg(dest_file.as_path().to_str().unwrap())
            .stdout(process::Stdio::null())
            .status()
            .expect("Failed to start process to pull files using adb");

        if status.success() {
            files_done.push(src_file)
        } else {
            files_failed.push(src_file)
        }
    }

    pb.finish();

    let success_path = PathBuf::from("./files_done.txt");
    let failed_path = PathBuf::from("./files_failed.txt");
    println!(
        "Done! Successfully copied {} files. Files written to {:?}",
        files_done.len(),
        success_path
    );

    if !files_failed.is_empty() {
        println!("Failed to copy {} files. Failed files written to {:?}", files_failed.len(), failed_path);
    }

    let mut file = OpenOptions::new().append(true).create(true).open(success_path.as_path()).unwrap();

    for path in files_done {
        if let Err(e) = writeln!(file, "{}", path.as_path().to_str().unwrap()) {
            eprintln!("Couldn't write to file: {}", e);
        }
    }

    if !files_failed.is_empty() {
        let mut file = OpenOptions::new().append(true).create(true).open(failed_path.as_path()).unwrap();

        for path in files_failed {
            if let Err(e) = writeln!(file, "{}", path.as_path().to_str().unwrap()) {
                eprintln!("Couldn't write to file: {}", e);
            }
        }
    }
}
