//! Ventoy IMG CLI

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

use ventoy_img::{VentoyImage, Result, VentoyError};

#[derive(Parser)]
#[command(name = "ventoy-img")]
#[command(version, about = "Create and manage Ventoy bootable IMG files")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Ventoy IMG file
    Create {
        /// Image size (e.g., 8G, 16G, 1024M)
        #[arg(short, long, default_value = "8G")]
        size: String,

        /// Output file path
        #[arg(short, long, default_value = "ventoy.img")]
        output: PathBuf,

        /// Volume label for data partition
        #[arg(short = 'L', long, default_value = "Ventoy")]
        label: String,
    },

    /// Add a file (ISO/IMG) to Ventoy image
    Add {
        /// Ventoy IMG file
        image: PathBuf,

        /// File to add
        file: PathBuf,

        /// Destination path in image (e.g., "iso/linux/ubuntu.iso")
        #[arg(short, long)]
        dest: Option<String>,

        /// Overwrite existing file
        #[arg(short, long)]
        force: bool,

        /// Create parent directories as needed
        #[arg(short, long)]
        parents: bool,
    },

    /// List files in Ventoy image
    List {
        /// Ventoy IMG file
        image: PathBuf,

        /// Directory path to list (default: root)
        #[arg(long)]
        path: Option<String>,

        /// List files recursively
        #[arg(short, long)]
        recursive: bool,
    },

    /// Remove a file or directory from Ventoy image
    Remove {
        /// Ventoy IMG file
        image: PathBuf,

        /// Path to remove (file or directory)
        path: String,

        /// Remove directories and their contents recursively
        #[arg(short, long)]
        recursive: bool,
    },

    /// Create a directory in Ventoy image
    Mkdir {
        /// Ventoy IMG file
        image: PathBuf,

        /// Directory path to create
        path: String,

        /// Create parent directories as needed
        #[arg(short, long)]
        parents: bool,
    },

    /// Show image information
    Info {
        /// Ventoy IMG file
        image: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Create { size, output, label } => cmd_create(&output, &size, &label),
        Commands::Add { image, file, dest, force, parents } => cmd_add(&image, &file, dest.as_deref(), force, parents),
        Commands::List { image, path, recursive } => cmd_list(&image, path.as_deref(), recursive),
        Commands::Remove { image, path, recursive } => cmd_remove(&image, &path, recursive),
        Commands::Mkdir { image, path, parents } => cmd_mkdir(&image, &path, parents),
        Commands::Info { image } => cmd_info(&image),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[ERROR] {}", e);
            ExitCode::FAILURE
        }
    }
}

fn cmd_create(output: &PathBuf, size: &str, label: &str) -> Result<()> {
    println!("========================================");
    println!("  Ventoy IMG Creator (Rust Edition)");
    println!("========================================");
    println!();

    VentoyImage::create(output, size, label)?;

    println!();
    println!("========================================");
    println!("Image: {}", output.display());
    println!("Size:  {}", size);
    println!("Label: {}", label);
    println!("========================================");

    Ok(())
}

fn cmd_add(image: &PathBuf, file: &PathBuf, dest: Option<&str>, force: bool, parents: bool) -> Result<()> {
    if !file.exists() {
        return Err(VentoyError::FileNotFound(file.display().to_string()));
    }

    let mut img = VentoyImage::open(image)?;

    match dest {
        Some(dest_path) => {
            // Add to specific path
            img.add_file_to_path(file, dest_path, parents, force)?;
            println!("Added {} -> {}", file.display(), dest_path);
        }
        None => {
            // Add to root
            if force {
                img.add_file_overwrite(file, true)?;
            } else {
                img.add_file(file)?;
            }
            println!("Added {}", file.display());
        }
    }

    Ok(())
}

fn cmd_list(image: &PathBuf, path: Option<&str>, recursive: bool) -> Result<()> {
    let img = VentoyImage::open(image)?;

    let files = if recursive {
        img.list_files_recursive()?
    } else {
        match path {
            Some(p) => img.list_files_at(p)?,
            None => img.list_files()?,
        }
    };

    if files.is_empty() {
        println!("No files in image");
        return Ok(());
    }

    if recursive {
        println!("{:<50} {:>15} {}", "PATH", "SIZE", "TYPE");
        println!("{}", "-".repeat(70));

        for file in files {
            let type_str = if file.is_directory { "DIR" } else { "FILE" };
            let size_str = format_size(file.size);
            println!("{:<50} {:>15} {}", file.path, size_str, type_str);
        }
    } else {
        println!("{:<40} {:>15} {}", "NAME", "SIZE", "TYPE");
        println!("{}", "-".repeat(60));

        for file in files {
            let type_str = if file.is_directory { "DIR" } else { "FILE" };
            let size_str = format_size(file.size);
            println!("{:<40} {:>15} {}", file.name, size_str, type_str);
        }
    }

    Ok(())
}

fn cmd_remove(image: &PathBuf, path: &str, recursive: bool) -> Result<()> {
    let mut img = VentoyImage::open(image)?;

    if recursive {
        img.remove_recursive(path)?;
        println!("Removed {} (recursive)", path);
    } else {
        img.remove_path(path)?;
        println!("Removed {}", path);
    }

    Ok(())
}

fn cmd_mkdir(image: &PathBuf, path: &str, parents: bool) -> Result<()> {
    let mut img = VentoyImage::open(image)?;
    img.create_directory(path, parents)?;
    println!("Created directory: {}", path);
    Ok(())
}

fn cmd_info(image: &PathBuf) -> Result<()> {
    let img = VentoyImage::open(image)?;
    let layout = img.layout();

    println!("Image: {}", image.display());
    println!();
    println!("Partition Layout:");
    println!("  Data partition:");
    println!("    Start:  sector {} (offset {})",
        layout.data_start_sector,
        format_size(layout.data_offset()));
    println!("    Size:   {} sectors ({})",
        layout.data_size_sectors,
        format_size(layout.data_size()));
    println!("  EFI partition:");
    println!("    Start:  sector {} (offset {})",
        layout.efi_start_sector,
        format_size(layout.efi_offset()));
    println!("    Size:   {} sectors (32 MB)",
        layout.efi_size_sectors);

    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
