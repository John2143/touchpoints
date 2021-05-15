fn main() {
    let arg = std::env::args().into_iter().skip(1).next().unwrap();
    if arg == "1" {
        println!("yep");
    } else if arg == "2" {
        std::fs::read_to_string("./src/main.rs").unwrap();
    } else {
        panic!("not impl")
    }
}
