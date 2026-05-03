use std::env;
use std::path::Path;
use vibe_index::VibeIndex;
use walkdir::WalkDir;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: {} <directory> <query>", args[0]);
        eprintln!();
        eprintln!("Indexes all .rs files in the given directory and searches for the query.");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} src \"fn main\"", args[0]);
        std::process::exit(1);
    }

    let target_dir = &args[1];
    let query = &args[2];

    let base = Path::new(target_dir);
    if !base.exists() {
        eprintln!("Error: directory '{}' does not exist", target_dir);
        std::process::exit(1);
    }

    let mut index = VibeIndex::new();
    let mut file_count = 0;

    for entry in WalkDir::new(base).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "rs" {
                    if let Ok(content) = std::fs::read_to_string(path) {
                        let relative = path
                            .strip_prefix(base)
                            .unwrap_or(path)
                            .to_string_lossy()
                            .replace('\\', "/");
                        index.add_file(&relative, &content);
                        file_count += 1;
                    }
                }
            }
        }
    }

    println!("Indexed {} .rs files", file_count);
    println!("Total tokens: {}", index.total_positions());
    println!("Unique tokens: {}", index.unique_tokens());
    println!();
    println!("Searching for: \"{}\"", query);
    println!("---");

    let results = index.search(query);

    if results.is_empty() {
        println!("No matches found.");
        return;
    }

    let mut last_file: Option<String> = None;
    for r in &results {
        let file = r.file_path.as_deref().unwrap_or("?");
        if file != last_file.as_deref().unwrap_or("") {
            if last_file.is_some() {
                println!();
            }
            println!("[{}] ({:.2} confidence)", file, r.confidence);
            last_file = Some(file.to_string());
        }

        let line_num = r.line_number.map(|n| format!(":{}", n)).unwrap_or_default();
        let snippet = r
            .highlighted_snippet
            .as_deref()
            .unwrap_or(r.line_content.as_deref().unwrap_or(""));
        println!("  {} {} {}", file, line_num, snippet);
    }

    println!();
    println!("---");
    println!("Total matches: {}", results.len());
}
