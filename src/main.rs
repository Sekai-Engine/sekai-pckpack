use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, Write},
    path::Path,
    process::Command,
};
use tempfile::tempdir;
use walkdir::WalkDir;
use zip::unstable::LittleEndianWriteExt;

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

    // 获取可执行文件所在目录
    let exe_dir = env::current_exe()
        .expect("Failed to get current executable path")
        .parent()
        .expect("Failed to get executable directory")
        .to_path_buf();

    // Process resource directories - args[2] to args[args.len()-2] (before -o flag)
    let mut resource_dirs = Vec::new();
    let mut i = 2;
    while i < args.len() && args[i] != "-o" {
        resource_dirs.push(&args[i]);
        i += 1;
    }

    let output_path = if i < args.len() {
        // Found "-o"
        if i + 1 < args.len() {
            Some(&args[i + 1])
        } else {
            eprintln!("Error: -o flag requires a value.");
            std::process::exit(1);
        }
    } else {
        None
    };

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
        let tool_name = if cfg!(target_os = "windows") {
            "godotpcktool.exe"
        } else {
            "godotpcktool"
        };
        let tool_path = exe_dir.join("tool").join(tool_name);
        if !tool_path.exists() {
            eprintln!(
                "Error: {} not found at {}",
                tool_name,
                tool_path.display()
            );
            std::process::exit(1);
        }

        println!("Using {} at {}", tool_name, tool_path.display());

        // 将资源文件复制到temp文件夹
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

        let pck_path = temp_dir.path().join("sekai.pck");
        let exe_name = if cfg!(target_os = "windows") {
            "sekai.exe"
        } else {
            "sekai"
        };
        let exe_copy = temp_dir.path().join(exe_name);

        extract_launcher(Path::new(&args[1]), &exe_copy, Some(&pck_path));

        let temp_resources_dir = resources_dir.to_str().unwrap();
        let mut command = Command::new(tool_path);
        command.arg(&pck_path.to_str().unwrap());
        command.args(["-a", "add", temp_resources_dir]);
        command.args(["--remove-prefix", temp_resources_dir]);
        match command.output() {
            Ok(output) => {
                if output.status.success() {
                    println!("PCK file created successfully");
                    if let Some(output_path) = output_path {
                        write_pck_to_exe(
                            exe_copy.as_path(),
                            pck_path.as_path(),
                            Path::new(output_path),
                        );
                    }
                } else {
                    eprintln!("Failed to create PCK file");
                    eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
                }
            }
            Err(e) => {
                eprintln!("Failed to execute godotpcktool: {}", e);
                std::process::exit(1);
            }
        };
    }
}

fn write_pck_to_exe(exe_path: &Path, pck_path: &Path, output_path: &Path) {
    let mut exe_file = File::open(exe_path).expect("Failed to open executable");
    let mut pck_file = File::open(pck_path).expect("Failed to open PCK file");
    let mut output_file = File::create(output_path).expect("Failed to create output file");

    std::io::copy(&mut exe_file, &mut output_file).expect("Failed to copy executable");
    let pck_size = std::io::copy(&mut pck_file, &mut output_file).expect("Failed to copy PCK");

    // Write PCK size (u64) and magic (u32)
    output_file.write_u64_le(pck_size).expect("Failed to write PCK size");
    output_file.write_all(b"GDPC").expect("Failed to write magic");
}

fn extract_launcher(input_path: &Path, output_path: &Path, pck_output_path: Option<&Path>) {
    let mut file = File::open(input_path).expect("Failed to open input executable");
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .expect("Failed to read input executable");

    let magic = b"GDPC";
    let len = buffer.len();
    let mut split_pos = None;

    // 1. Try to find footer magic
    if len >= 12 {
        let footer_magic = &buffer[len - 4..];
        if footer_magic == magic {
            let size_bytes = &buffer[len - 12..len - 4];
            let pck_size = u64::from_le_bytes(size_bytes.try_into().unwrap()) as usize;
            
            // Check if size is reasonable and matches header
            let start_pos = len - 12 - pck_size;
            if start_pos < len && &buffer[start_pos..start_pos + 4] == magic {
                println!("Found PCK via footer size. Size: {}", pck_size);
                split_pos = Some(start_pos);
            }
        }
    }

    // 2. Fallback to searching for the first GDPC occurrence
    if split_pos.is_none() {
        println!("Footer not found or invalid, searching for magic header...");
        split_pos = buffer
            .windows(magic.len())
            .position(|window| window == magic);
    }

    let split_index = split_pos.unwrap_or(len);

    let mut output = File::create(output_path).expect("Failed to create output launcher");
    output
        .write_all(&buffer[..split_index])
        .expect("Failed to write launcher");
    println!("Extracted launcher size: {} bytes", split_index);

    if let Some(pck_out) = pck_output_path {
        if split_index < buffer.len() {
            let mut pck_file = File::create(pck_out).expect("Failed to create output PCK");
            // If we found via footer, the PCK data is between split_index and (len - 12)
            // But we might want to keep it simple and write everything, OR strip the footer?
            // Usually tools expect clean PCK. Let's strip the footer if we found it via footer logic.
            
            let end_index = if len >= 12 && &buffer[len - 4..] == magic && split_index == len - 12 - (u64::from_le_bytes(buffer[len - 12..len - 4].try_into().unwrap()) as usize) {
                len - 12
            } else {
                len
            };

            pck_file
                .write_all(&buffer[split_index..end_index])
                .expect("Failed to write PCK");
            println!("Extracted PCK size: {} bytes", end_index - split_index);
        } else {
            println!("No embedded PCK found in executable.");
        }
    }
}
