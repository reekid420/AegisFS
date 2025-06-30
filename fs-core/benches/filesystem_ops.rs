use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::fs::{File, remove_file};
use std::io::{Write, Read};
use tempfile::TempDir;

fn benchmark_file_operations(c: &mut Criterion) {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("benchmark.txt");
    
    c.bench_function("file_create_write_1kb", |b| {
        let data = vec![0u8; 1024];
        b.iter(|| {
            let mut file = File::create(&file_path).unwrap();
            file.write_all(black_box(&data)).unwrap();
            file.sync_all().unwrap();
            remove_file(&file_path).unwrap();
        });
    });
    
    // Create a test file for read benchmark
    let mut file = File::create(&file_path).unwrap();
    let data = vec![42u8; 1024];
    file.write_all(&data).unwrap();
    file.sync_all().unwrap();
    drop(file);
    
    c.bench_function("file_read_1kb", |b| {
        b.iter(|| {
            let mut file = File::open(&file_path).unwrap();
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer).unwrap();
            black_box(buffer);
        });
    });
}

criterion_group!(benches, benchmark_file_operations);
criterion_main!(benches); 