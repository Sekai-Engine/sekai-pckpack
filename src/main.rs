use std::{
    env,
    fs::{self, File},
    io::{self, Read, Seek, Write},
    path::Path,
};
use tempfile::tempdir;
use zip::write::FileOptions;

fn zip_resource_files(resource_dirs: &Vec<&String>, target_path: &Path) -> io::Result<()> {
    let inner = File::create(target_path)?;
    let mut zip = zip::ZipWriter::new(inner);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    // Add each resource directory to the zip file
    for (_idx, dir_path) in resource_dirs.iter().enumerate() {
        let source_path = Path::new(dir_path);
        if !source_path.exists() || !source_path.is_dir() {
            continue; // Skip non-existent or non-directory paths
        }

        // Add directory contents to zip with prefix
        let dir_name = source_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        add_dir_to_zip(&mut zip, source_path, &format!("{}/", dir_name), options)?;
    }

    zip.finish()?;
    Ok(())
}

/// Add directory contents to zip file recursively
fn add_dir_to_zip(
    zip: &mut zip::ZipWriter<File>,
    source_dir: &Path,
    prefix: &str,
    options: FileOptions<()>,
) -> io::Result<()> {
    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let zip_path = format!("{}{}", prefix, file_name.to_string_lossy());

        if path.is_dir() {
            // Add subdirectory recursively
            add_dir_to_zip(zip, &path, &format!("{}/", zip_path), options)?;
        } else {
            // Add file to zip
            zip.start_file(zip_path, options)?;
            let mut file = File::open(&path)?;
            std::io::copy(&mut file, zip)?;
        }
    }
    Ok(())
}

fn write_into_main_exe(exe_path: &str, zip_path: &str, output_path: &str) -> io::Result<()> {
    // 1. 打开文件
    let mut exe_file = File::open(exe_path)?;
    let mut zip_file = File::open(zip_path)?;
    let mut output_file = File::create(output_path)?;

    // 2. 获取源 EXE 的总长度
    let exe_len = exe_file.metadata()?.len();
    if exe_len < 8 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Source file too small, unable to read Footer",
        ));
    }

    // 3. 读取源 EXE 最后 8 个字节（获取旧的 Size）
    exe_file.seek(io::SeekFrom::End(-8))?;
    let mut old_size_buffer = [0u8; 8];
    exe_file.read_exact(&mut old_size_buffer)?;
    let old_size = u64::from_le_bytes(old_size_buffer);

    // 4. 复制源 EXE 的内容（**去掉**最后 8 个字节）
    // 我们需要把指针移回开头
    exe_file.seek(io::SeekFrom::Start(0))?;
    // 使用 take() 只读取前 (总长度 - 8) 个字节
    // 这样就相当于把旧的 Footer "删掉" 了，空出了位置给新 Zip
    let mut exe_reader = exe_file.take(exe_len - 8);
    io::copy(&mut exe_reader, &mut output_file)?;

    // 5. 写入新的 Zip 内容，并获取新 Zip 的大小
    let new_zip_size = io::copy(&mut zip_file, &mut output_file)?;

    // 6. 计算新的 Footer 数值 (旧大小 + 新大小)
    let final_size = old_size + new_zip_size;

    // 7. 写入新的 Footer (8字节)
    output_file.write_all(&final_size.to_le_bytes())?;

    println!("Injection successful!");
    println!("Original Footer value: {}", old_size);
    println!("New Zip size: {}", new_zip_size);
    println!("New Footer value: {}", final_size);

    Ok(())
}

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
        let zip_output_path = temp_dir.path().join("sekai-resource.zip");

        match zip_resource_files(&resource_dirs, &zip_output_path) {
            Ok(_) => {
                println!(
                    "Successfully created temporary resource archive: {:?}",
                    zip_output_path
                );
                // Copy the zip file to test_env for debugging
                // let debug_path = Path::new("d:/Godot/sekai-pack/test_env/debug_resource.zip");
                // match fs::copy(&zip_output_path, &debug_path) {
                //     Ok(_) => println!("Debug zip copied to: {:?}", debug_path),
                //     Err(e) => eprintln!("Warning: Failed to copy debug zip: {}", e),
                // }
            }
            Err(e) => {
                eprintln!("Error creating resource archive: {}", e);
                std::process::exit(1);
            }
        }

        let output_path = if i < args.len() && args[i] == "-o" && i + 1 < args.len() {
            Path::new(&args[i + 1]).to_path_buf()
        } else {
            eprintln!("No output path specified");
            std::process::exit(1);
        };

        match write_into_main_exe(
            &args[1],
            &zip_output_path.to_str().unwrap().to_string(),
            &output_path.to_str().unwrap().to_string(),
        ) {
            Ok(_) => println!("Successfully wrote resource archive into main executable"),
            Err(e) => {
                eprintln!("Error writing resource archive into main executable: {}", e);
                std::process::exit(1);
            }
        };
    }
}
