// Diagnostic: run the real `list_programs()` and report size coverage.
use svalka_core::programs::list_programs;

fn main() {
    let progs = list_programs();
    let with_size = progs.iter().filter(|p| p.real_size > 0).count();
    println!("total programs   : {}", progs.len());
    println!("with real_size>0 : {}", with_size);
    println!("--- sample ---");
    for p in &progs {
        let n = &p.name;
        if n.contains("Bandicam")
            || n.contains("Dota 2")
            || n.contains("FileZilla")
            || n.contains("007 First")
            || n.contains("NVIDIA")
        {
            println!("{:>14} bytes | loc={:?} | {}", p.real_size, p.install_location, n);
        }
    }
}
