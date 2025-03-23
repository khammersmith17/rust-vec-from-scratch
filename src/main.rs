mod my_vec;

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn add() {}

    #[test]
    fn pop() {}
}
