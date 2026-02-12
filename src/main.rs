use std::{
    env,
    fs::{self, File},
    io::Write,
    path::Path,
    process::Command,
};
use tempfile::tempdir;
use walkdir::WalkDir;

const GODOT_PCK_TOOL: &[u8] = include_bytes!("../bin/godotpcktool.exe");

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("AppBinder v1.0 - Package applications with resources");
        eprintln!(
            "Usage: {} <main_executable> [resource_dirs...] [-o output]",
            args[0]
        );
        eprintln!(
            "Example: {} test_env/sekai.x86_64 test_env/script test_env/sounds -o bundled_sekai",
            args[0]
        );
        std::process::exit(1);
    }

    // Process resource directories - args[2] to args[args.len()-2] (before -o flag)
    let mut resource_dirs = Vec::new();
    let mut i = 2;

    while i < args.len() && args[i] != "-o" {
        resource_dirs.push(&args[i]);
        i += 1;
    }

    if resource_dirs.is_empty() {
        println!("No resource directories specified");
    } else {
        println!("Processing {} resource directories", resource_dirs.len());

        let temp_dir = match tempdir() {
            Ok(dir) => dir,
            Err(e) => {
                eprintln!("Failed to create temporary directory: {}", e);
                std::process::exit(1);
            }
        };
        let tool_path = temp_dir.path().join("godotpcktool.exe");
        {
            let mut godot_pck_tool_file = match File::create(&tool_path) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Failed to create godotpcktool.exe: {}", e);
                    std::process::exit(1);
                }
            };
            godot_pck_tool_file.write_all(GODOT_PCK_TOOL).unwrap();
        }

        println!("godotpcktool.exe written to {}", tool_path.display());

        let resources_dir = temp_dir.path().join("sekai-resources");
        fs::create_dir_all(&resources_dir).unwrap();

        for dir in &resource_dirs {
            println!("Processing resource directory: {}", dir);
            let source_path = Path::new(dir);
            if !source_path.exists() {
                eprintln!("Warning: Source directory does not exist: {}", dir);
                continue;
            }

            let dir_name = source_path.file_name().unwrap();
            let dest_base = resources_dir.join(dir_name);

            for entry in WalkDir::new(source_path) {
                let entry = entry.unwrap();
                let path = entry.path();
                let rel_path = path.strip_prefix(source_path).unwrap();
                let target_path = dest_base.join(rel_path);

                if path.is_dir() {
                    fs::create_dir_all(&target_path).unwrap();
                } else {
                    if let Some(parent) = target_path.parent() {
                        fs::create_dir_all(parent).unwrap();
                    }
                    fs::copy(path, &target_path).unwrap();
                }
            }
        }
        let mut command = Command::new(tool_path);
        command.arg("sekai.pck");
        command.args(["-a", "a", resources_dir.to_str().unwrap()]);
        command.args(["--remove-prefix", "sekai-resources"]);
        let output = command.output().unwrap();
        if output.status.success() {
            println!("PCK file created successfully");
        } else {
            eprintln!("Failed to create PCK file");
        }
    }
}
