use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vibe_index::VibeIndex;

fn generate_file_content(num_tokens: usize) -> String {
    let common_tokens = [
        "fn", "let", "mut", "if", "else", "for", "while", "return", "match", "use", "pub",
        "struct", "impl", "trait", "enum", "async", "await", "self", "Self", "String", "Vec",
        "Option", "Result", "Some", "None", "Ok", "Err", "true", "false", "(", ")", "{", "}",
        ";", ":", ",", ".", "=>",
    ];

    let mut content = String::new();
    let mut line_tokens = Vec::new();

    for i in 0..num_tokens {
        let token = &common_tokens[i % common_tokens.len()];
        line_tokens.push(token.to_string());

        if line_tokens.len() >= 8 {
            content.push_str(&line_tokens.join(" "));
            content.push('\n');
            line_tokens.clear();
        }
    }

    if !line_tokens.is_empty() {
        content.push_str(&line_tokens.join(" "));
        content.push('\n');
    }

    content
}

   fn bench_file_index_lookup(c: &mut Criterion) {
        // Test file lookup performance at different scales
        for &num_files in &[10, 50, 100, 500] {
            let mut index = VibeIndex::new();
            let tokens_per_file = 100;

            for file_idx in 0..num_files {
                let path = format!("src/module_{:04}.rs", file_idx);
                let content = generate_file_content(tokens_per_file);
                index.add_file(&path, &content);
            }

            // Find a token that exists in the middle file
            let search_token = "fn";

            c.bench_function(
                &format!("file_lookup_{}files_phrase_search", num_files),
                |b| {
                    b.iter(|| {
                        let results = index.phrase_search(&[search_token.into(), "let".into()]);
                        let count = results.len();
                        black_box(count)
                    })
                },
            );
        }
    }

    fn bench_file_index_lookup_with_binary_search(c: &mut Criterion) {
        // Test binary search file lookup performance at different scales
        for &num_files in &[10, 50, 100, 500] {
            let mut index = VibeIndex::new();
            let tokens_per_file = 100;

            for file_idx in 0..num_files {
                let path = format!("src/module_{:04}.rs", file_idx);
                let content = generate_file_content(tokens_per_file);
                index.add_file(&path, &content);
            }

            // Build the binary search lookup index
            index.file_index.build_lookup_index();

            let search_token = "fn";

            c.bench_function(
                &format!("file_lookup_{}files_binary_search", num_files),
                |b| {
                    b.iter(|| {
                        let results = index.phrase_search(&[search_token.into(), "let".into()]);
                        let count = results.len();
                        black_box(count)
                    })
                },
            );
        }
    }

fn bench_file_index_add_file(c: &mut Criterion) {
    let content = generate_file_content(1000);

    for &num_files in &[10, 50, 100, 500] {
        c.bench_function(
            &format!("add_file_{}files", num_files),
            |b| {
                b.iter(|| {
                    let mut index = VibeIndex::new();
                    for file_idx in 0..num_files {
                        let path = format!("src/module_{:04}.rs", file_idx);
                        index.add_file(&path, &content);
                    }
                    black_box(index)
                })
            },
        );
    }
}

fn bench_file_index_persistence(c: &mut Criterion) {
    let temp_path = "bench_file_index_persist.bin";
    let _ = std::fs::remove_file(temp_path);

    let mut group = c.benchmark_group("file_persistence");

    for &num_files in &[10, 50, 100] {
        // Save
        {
            let mut storage = vibe_index::persistent_storage::PersistentStorage::new(temp_path);
            for file_idx in 0..num_files {
                let path = format!("src/module_{:04}.rs", file_idx);
                let content = generate_file_content(100);
                storage.add_file(&path, &content);
            }
            storage.save().unwrap();
        }

        let file_size = std::fs::metadata(temp_path).unwrap().len();

        group.bench_function(
            format!("save_{}files_{}KB", num_files, file_size / 1024),
            |b| {
                b.iter(|| {
                    let mut storage = vibe_index::persistent_storage::PersistentStorage::new(temp_path);
                    for file_idx in 0..num_files {
                        let path = format!("src/module_{:04}.rs", file_idx);
                        let content = generate_file_content(100);
                        storage.add_file(&path, &content);
                    }
                    storage.save().unwrap();
                    black_box(())
                })
            },
        );

        group.bench_function(
            format!("load_{}files_{}KB", num_files, file_size / 1024),
            |b| {
                b.iter(|| {
                    let loaded = vibe_index::persistent_storage::PersistentStorage::load(temp_path);
                    black_box(loaded.total_files())
                })
            },
        );
    }

    group.finish();
    let _ = std::fs::remove_file(temp_path);
}

fn bench_file_index_search_with_metadata(c: &mut Criterion) {
    let mut index = VibeIndex::new();
    let num_files = 100;
    let tokens_per_file = 200;

    for file_idx in 0..num_files {
        let path = format!("src/module_{:04}.rs", file_idx);
        let content = generate_file_content(tokens_per_file);
        index.add_file(&path, &content);
    }

    c.bench_function("search_100files_with_file_metadata", |b| {
        b.iter(|| {
            let results = index.phrase_search(&["fn".into(), "let".into()]);
            // Verify file metadata is populated
            let with_file_info: usize = results
                .iter()
                .filter(|r| r.file_path.is_some() && r.line_number.is_some())
                .count();
            black_box(with_file_info)
        })
    });
}

criterion_group!(
    benches,
    bench_file_index_lookup,
    bench_file_index_lookup_with_binary_search,
    bench_file_index_add_file,
    bench_file_index_persistence,
    bench_file_index_search_with_metadata,
);
criterion_main!(benches);
