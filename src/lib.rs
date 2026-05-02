pub mod graphics_context;
pub mod lazy_graphics_resource;
pub mod math;
pub mod memory;
pub mod renderer;
pub mod resources;
pub mod svg;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
