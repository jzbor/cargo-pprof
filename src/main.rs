use std::{env, fmt::Display, fs::{self, File}, io::BufRead, path::Path, process};

use clap::{Parser, Subcommand};
use colored::Colorize;
use serde::Deserialize;
use std::io::Write;

pub const CARGO_TOML_SNIPPET: &str = include_str!("cargo-toml-snippet.toml");

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Profile Rust applications with perf
    #[clap(name = "pprof")]
    PProf(#[clap(flatten)] PProfArgs),
}

#[derive(Parser, Debug)]
struct PProfArgs {
    /// Add "profiling" profile to Cargo.toml (simple append)
    #[clap(long)]
    add: bool,

    /// Open the firefox profiler and exit
    #[clap(short, long)]
    open_firefox_profiler: bool,

    /// Ignore exit code of the profiled application
    #[clap(short, long)]
    ignore_exit: bool,

    /// Arguments that are passed to the profiled application
    #[clap(last(true))]
    app_args: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct CompilerMessage {
    executable: String,
}


fn resolve<T, E: Display>(result: Result<T, E>) -> T {
    match result {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1)
        },
    }
}

fn resolve_status(status: process::ExitStatus) {
    if !status.success() {
        if let Some(code) = status.code() {
            let _ign: u32 = resolve(Err(format!("Cargo returned with exit code {}", code)));
        } else {
            let _ign: u32 = resolve(Err("Cargo returned with an error".to_string()));
        }
    }
}

fn print_step(desc: &str) {
    let msg = format!("=> {}", desc);
    eprintln!("\n{}", msg.green().bold());
}

fn open_firefox_profiler() {
    let status = resolve(process::Command::new("firefox")
        .arg("https://profiler.firefox.com")
        .status());
    resolve_status(status);
}

fn add_to_cargo_toml() {
    print_step("Appending snippet to Cargo.toml");
    let mut file = resolve(fs::OpenOptions::new()
        .append(true)
        .open("Cargo.toml"));
    eprintln!("Done");
    resolve(write!(file, "\n{}", CARGO_TOML_SNIPPET));
}


fn main() {
    let Command::PProf(args) = Args::parse().command;

    if args.open_firefox_profiler {
        open_firefox_profiler();
        process::exit(0);
    } else if args.add {
        add_to_cargo_toml();
        process::exit(0);
    }


    let cargo_path = resolve(env::var("CARGO"));

    print_step("Building binary");
    let cargo_out = resolve(process::Command::new(cargo_path)
        .arg("build")
        .arg("--message-format=json-render-diagnostics")
        .arg("--profile=profiling")
        .stderr(process::Stdio::inherit())
        .output());
    resolve_status(cargo_out.status);
    let lines = cargo_out.stdout.lines()
        .map_while(Result::ok);
    let messages = lines.flat_map(|l| serde_json::from_str::<CompilerMessage>(&l));
    let executable = match messages.last() {
        Some(msg) => msg.executable.clone(),
        None => resolve(Err("Could not find executable".to_string())),
    };
    let dir = match Path::new(&executable).parent() {
        Some(dir) => dir,
        None => resolve(Err("Could not determine output directory")),
    };
    let perf_out_path = dir.join("perf.data");
    let trace_path = dir.join("perf.trace");
    eprintln!("Binary found: {}", executable);

    print_step("Running program with perf");
    let status = resolve(process::Command::new("perf")
        .arg("record")
        .arg(format!("--output={}", perf_out_path.to_string_lossy()))
        .args(["-g", "-F", "999"])
        .arg(executable)
        .args(args.app_args)
        .status());
    if !args.ignore_exit {
        resolve_status(status);
    }

    print_step("Converting data to trace format");
    let trace_file = resolve(File::create(&trace_path));
    let status = resolve(process::Command::new("perf")
        .arg("script")
        .args(["-F", "+pid"])
        .arg(format!("--input={}", perf_out_path.to_string_lossy()))
        .stdout(process::Stdio::from(trace_file))
        .status());
    resolve_status(status);
    println!("Trace file: {}", trace_path.to_string_lossy().cyan());
    println!("This file can be viewed using the Firefox Profiler ({})", "https://profiler.firefox.com".bright_blue());
}
